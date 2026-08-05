use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::crypto::{derive_file_key, verify_signature, xor_crypt};
use crate::machine_id::collect_machine_id;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TRIAL_DAYS: u64 = 7;
/// Allow up to 5 minutes clock skew before flagging time manipulation.
const TIME_TOLERANCE_SECS: u64 = 300;

// ---------------------------------------------------------------------------
// Persisted data structures
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LicenseFile {
    pub v: u32,
    pub machine_id: String,
    pub expiry: u64, // Unix timestamp; 0 = lifetime
    pub edition: String,
    pub sig: String,     // Base64 Ed25519 signature
    pub last_run: u64,   // Updated every launch to detect clock rollbacks
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TrialFile {
    pub v: u32,
    pub machine_id: String,
    pub trial_started: u64,
    pub last_run: u64,
}

// ---------------------------------------------------------------------------
// Public status type — serialized to JSON for the frontend
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "status")]
pub enum LicenseStatus {
    /// No license and no trial started.
    Unlicensed { machine_id: String },
    /// Trial in progress.
    Trial { machine_id: String, days_left: i64 },
    /// Trial period has ended, no license.
    TrialExpired { machine_id: String },
    /// Valid license but past expiry.
    Expired { machine_id: String, edition: String },
    /// Fully licensed and active.
    Active {
        machine_id: String,
        edition: String,
        expiry_str: String,
        days_left: Option<i64>, // None for lifetime
    },
    /// System clock appears to have been rolled back.
    TimeManipulation { machine_id: String },
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn app_data_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn license_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join("license.dat")
}

fn trial_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join("trial.dat")
}

fn runtime_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join("runtime.enc")
}

// ---------------------------------------------------------------------------
// Encrypted file I/O helpers
// ---------------------------------------------------------------------------

fn load_encrypted<T: for<'de> Deserialize<'de>>(
    path: &PathBuf,
    machine_id: &str,
) -> Option<T> {
    let encrypted = std::fs::read(path).ok()?;
    let key = derive_file_key(machine_id);
    let decrypted = xor_crypt(&encrypted, &key);
    serde_json::from_slice(&decrypted).ok()
}

fn save_encrypted<T: Serialize>(
    path: &PathBuf,
    machine_id: &str,
    value: &T,
) -> Result<(), String> {
    let json = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let key = derive_file_key(machine_id);
    let encrypted = xor_crypt(&json, &key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, encrypted).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// runtime.enc — stores last_run as a UTF-8 decimal string
// ---------------------------------------------------------------------------

fn read_last_run(app: &AppHandle, machine_id: &str) -> Option<u64> {
    let path = runtime_path(app);
    let encrypted = std::fs::read(&path).ok()?;
    let key = derive_file_key(machine_id);
    let decrypted = xor_crypt(&encrypted, &key);
    std::str::from_utf8(&decrypted)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

fn write_last_run(app: &AppHandle, machine_id: &str, ts: u64) {
    let path = runtime_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let key = derive_file_key(machine_id);
    let encrypted = xor_crypt(ts.to_string().as_bytes(), &key);
    let _ = std::fs::write(&path, encrypted);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn normalize_license_key(key: &str) -> String {
    let cleaned: String = key
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    let parts: Vec<&str> = cleaned.splitn(5, '|').collect();
    if parts.len() != 5 {
        return cleaned;
    }

    let sig = parts[4].replace('-', "");
    format!(
        "{}|{}|{}|{}|{}",
        parts[0].trim().to_uppercase(),
        parts[1].trim(),
        parts[2].trim(),
        parts[3].trim(),
        sig
    )
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn format_expiry(expiry: u64, now: u64) -> String {
    if expiry == 0 {
        return "Vĩnh viễn".to_string();
    }
    if now >= expiry {
        return "Đã hết hạn".to_string();
    }
    let days = (expiry - now) / 86400;
    format!("Còn {days} ngày")
}

fn build_payload(machine_id: &str, expiry: u64, edition: &str) -> String {
    format!("HHPOS|{machine_id}|{expiry}|{edition}")
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check current license status. Called every app launch.
pub fn check_license(app: &AppHandle) -> LicenseStatus {
    let machine_id = collect_machine_id();
    let now = now_secs();

    // --- Anti-time-manipulation: compare now with stored last_run ---
    if let Some(last_run) = read_last_run(app, &machine_id) {
        if last_run > now + TIME_TOLERANCE_SECS {
            return LicenseStatus::TimeManipulation {
                machine_id,
            };
        }
    }

    // --- Try active license ---
    let lic_path = license_path(app);
    if let Some(lic) = load_encrypted::<LicenseFile>(&lic_path, &machine_id) {
        if lic.machine_id == machine_id {
            let payload = build_payload(&lic.machine_id, lic.expiry, &lic.edition);
            if verify_signature(&payload, &lic.sig) {
                // Expired?
                if lic.expiry != 0 && now > lic.expiry {
                    write_last_run(app, &machine_id, now);
                    return LicenseStatus::Expired {
                        machine_id,
                        edition: lic.edition,
                    };
                }
                // Active — update last_run
                let days_left = if lic.expiry == 0 {
                    None
                } else {
                    Some((lic.expiry as i64 - now as i64) / 86400)
                };
                let expiry_str = format_expiry(lic.expiry, now);
                let mut updated = lic;
                updated.last_run = now;
                let _ = save_encrypted(&lic_path, &machine_id, &updated);
                write_last_run(app, &machine_id, now);
                return LicenseStatus::Active {
                    machine_id,
                    edition: updated.edition,
                    expiry_str,
                    days_left,
                };
            }
        }
    }

    // --- Try trial ---
    let trial_path = trial_path(app);
    if let Some(trial) = load_encrypted::<TrialFile>(&trial_path, &machine_id) {
        if trial.machine_id == machine_id {
            let elapsed_days = (now.saturating_sub(trial.trial_started)) / 86400;
            let days_left = TRIAL_DAYS as i64 - elapsed_days as i64;
            // Update last_run
            let mut updated = trial;
            updated.last_run = now;
            let _ = save_encrypted(&trial_path, &machine_id, &updated);
            write_last_run(app, &machine_id, now);
            if days_left > 0 {
                return LicenseStatus::Trial {
                    machine_id,
                    days_left,
                };
            } else {
                return LicenseStatus::TrialExpired { machine_id };
            }
        }
    }

    write_last_run(app, &machine_id, now);
    LicenseStatus::Unlicensed { machine_id }
}

/// Activate the app with a license key string.
///
/// Expected format: `HHPOS|{machine_id}|{expiry_unix}|{edition}|{base64_sig}`
pub fn activate_license(app: &AppHandle, key: &str) -> Result<LicenseStatus, String> {
    let key = normalize_license_key(key);
    let parts: Vec<&str> = key.splitn(5, '|').collect();

    if parts.len() != 5 || parts[0] != "HHPOS" {
        return Err(
            "Định dạng key không hợp lệ.\nKey phải có dạng: HHPOS|MachineID|Expiry|Edition|Signature"
                .to_string(),
        );
    }

    let key_machine = parts[1].trim();
    let expiry: u64 = parts[2]
        .parse()
        .map_err(|_| "Key bị lỗi: trường expiry không hợp lệ.".to_string())?;
    let edition = parts[3].trim().to_uppercase();
    let sig = parts[4].trim();

    let machine_id = collect_machine_id();

    if !key_machine.eq_ignore_ascii_case(&machine_id) {
        return Err(format!(
            "Key này không dành cho máy của bạn.\n\
             Mã máy của bạn  : {machine_id}\n\
             Mã máy trong key: {key_machine}"
        ));
    }

    let valid_editions = ["FREE", "BASIC", "PRO", "LIFETIME"];
    if !valid_editions.contains(&edition.as_str()) {
        return Err("Edition không hợp lệ. Phải là: FREE, BASIC, PRO hoặc LIFETIME.".to_string());
    }

    let payload = build_payload(key_machine, expiry, &edition);
    if !verify_signature(&payload, sig) {
        return Err(
            "Chữ ký số không hợp lệ. Key có thể đã bị chỉnh sửa hoặc bị lỗi.".to_string(),
        );
    }

    let now = now_secs();
    if expiry != 0 && now > expiry {
        return Err("License này đã hết hạn từ trước khi kích hoạt.".to_string());
    }

    let lic = LicenseFile {
        v: 1,
        machine_id: machine_id.clone(),
        expiry,
        edition: edition.clone(),
        sig: sig.to_string(),
        last_run: now,
    };
    save_encrypted(&license_path(app), &machine_id, &lic)?;
    write_last_run(app, &machine_id, now);

    let days_left = if expiry == 0 {
        None
    } else {
        Some((expiry as i64 - now as i64) / 86400)
    };

    Ok(LicenseStatus::Active {
        machine_id,
        edition,
        expiry_str: format_expiry(expiry, now),
        days_left,
    })
}

/// Start the 7-day free trial (one-time per machine).
pub fn start_trial(app: &AppHandle) -> Result<LicenseStatus, String> {
    let machine_id = collect_machine_id();

    // Reject if a valid license already exists
    let lic_path = license_path(app);
    if let Some(lic) = load_encrypted::<LicenseFile>(&lic_path, &machine_id) {
        if lic.machine_id == machine_id {
            let payload = build_payload(&lic.machine_id, lic.expiry, &lic.edition);
            if verify_signature(&payload, &lic.sig) {
                return Err("Máy này đã có bản quyền hợp lệ. Không cần dùng thử.".to_string());
            }
        }
    }

    // Reject if trial already used
    let t_path = trial_path(app);
    if let Some(trial) = load_encrypted::<TrialFile>(&t_path, &machine_id) {
        if trial.machine_id == machine_id {
            let now = now_secs();
            let elapsed_days = (now.saturating_sub(trial.trial_started)) / 86400;
            if elapsed_days < TRIAL_DAYS {
                let left = TRIAL_DAYS as i64 - elapsed_days as i64;
                return Err(format!(
                    "Bạn đang trong thời gian dùng thử. Còn {left} ngày."
                ));
            }
            return Err(
                "Thời gian dùng thử đã hết. Vui lòng liên hệ để mua bản quyền.".to_string(),
            );
        }
    }

    let now = now_secs();
    let trial = TrialFile {
        v: 1,
        machine_id: machine_id.clone(),
        trial_started: now,
        last_run: now,
    };
    save_encrypted(&t_path, &machine_id, &trial)?;
    write_last_run(app, &machine_id, now);

    Ok(LicenseStatus::Trial {
        machine_id,
        days_left: TRIAL_DAYS as i64,
    })
}
