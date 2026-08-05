#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use eframe::{egui, App, CreationContext, NativeOptions};
use rfd::FileDialog;

const EDITIONS: [&str; 4] = ["FREE", "BASIC", "PRO", "LIFETIME"];

#[derive(Clone, PartialEq)]
enum MainTab {
    KeysGenerator,
    Setup,
}

#[derive(Clone)]
struct LicenseLogEntry {
    time: String,
    customer_name: String,
    machine_id: String,
    edition: String,
    expiry: String,
    license_key: String,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn format_local_time() -> String {
    let secs = now_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;
    // Approx date from epoch (good enough for log)
    let mut y = 1970u64;
    let mut rem = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let diy = if leap { 366 } else { 365 };
        if rem < diy {
            break;
        }
        rem -= diy;
        y += 1;
    }
    let month_days: [u64; 12] = {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 1u64;
    let mut d = rem + 1;
    for md in month_days {
        if d <= md {
            break;
        }
        d -= md;
        mo += 1;
    }
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

fn expiry_to_display(expiry: u64) -> String {
    if expiry == 0 {
        return "Vĩnh viễn".to_string();
    }
    let days_since_epoch = expiry / 86400;
    let mut y = 1970u64;
    let mut remaining = days_since_epoch;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days: [u64; 12] = {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    let mut d = remaining + 1;
    for days in month_days.iter() {
        if d <= *days {
            break;
        }
        d -= days;
        m += 1;
    }
    format!("{y:04}-{:02}-{d:02}", m + 1)
}

/// Format key for display (optional hyphen grouping on signature part).
fn format_key_display(raw: &str, add_hyphens: bool) -> String {
    if !add_hyphens || raw.is_empty() {
        return raw.to_string();
    }
    let parts: Vec<&str> = raw.splitn(5, '|').collect();
    if parts.len() != 5 {
        return raw.to_string();
    }
    let sig = parts[4];
    let sig_hyphen: String = sig
        .chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-");
    format!(
        "{}|{}|{}|{}|{}",
        parts[0], parts[1], parts[2], parts[3], sig_hyphen
    )
}

fn create_key_pair(output_dir: &str) -> Result<(String, String, String), String> {
    let out = PathBuf::from(output_dir);
    fs::create_dir_all(&out).map_err(|e| format!("Không tạo được thư mục: {e}"))?;

    let mut rng = rand::rngs::OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    let verifying_key = signing_key.verifying_key();

    let priv_path = out.join("ed25519_private.key");
    let pub_path = out.join("ed25519_public.key");

    fs::write(&priv_path, signing_key.to_bytes())
        .map_err(|e| format!("Không ghi private key: {e}"))?;
    fs::write(&pub_path, verifying_key.to_bytes())
        .map_err(|e| format!("Không ghi public key: {e}"))?;

    let pub_bytes = verifying_key
        .to_bytes()
        .iter()
        .map(|b| format!("0x{b:02x}"))
        .collect::<Vec<_>>()
        .join(", ");

    Ok((
        priv_path.to_string_lossy().to_string(),
        pub_path.to_string_lossy().to_string(),
        format!("[{pub_bytes}]"),
    ))
}

fn generate_license_key(
    machine_id: &str,
    edition: &str,
    days: i64,
    privkey_path: &str,
) -> Result<(String, u64), String> {
    if machine_id.trim().is_empty() {
        return Err("Hardware ID (Mã máy) không được để trống.".to_string());
    }
    if !EDITIONS.contains(&edition) {
        return Err("Edition không hợp lệ.".to_string());
    }

    let priv_bytes_vec =
        fs::read(privkey_path).map_err(|e| format!("Không đọc được private key: {e}"))?;
    let priv_bytes: [u8; 32] = priv_bytes_vec
        .try_into()
        .map_err(|_| "Private key phải đúng 32 bytes.".to_string())?;
    let signing_key = SigningKey::from_bytes(&priv_bytes);

    let expiry: u64 = if days < 0 {
        0
    } else {
        now_secs() + (days as u64) * 86400
    };

    let payload = format!("HHPOS|{}|{}|{}", machine_id.trim(), expiry, edition);
    let signature = signing_key.sign(payload.as_bytes());
    let sig_b64 = STANDARD.encode(signature.to_bytes());
    Ok((format!("{payload}|{sig_b64}"), expiry))
}

fn normalize_key_for_verify(key: &str) -> String {
    let cleaned: String = key
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let parts: Vec<&str> = cleaned.splitn(5, '|').collect();
    if parts.len() == 5 {
        let sig = parts[4].replace('-', "");
        format!(
            "{}|{}|{}|{}|{}",
            parts[0].trim().to_uppercase(),
            parts[1].trim(),
            parts[2].trim(),
            parts[3].trim(),
            sig
        )
    } else {
        cleaned
    }
}

fn verify_license_key(key: &str, pubkey_path: &str) -> Result<String, String> {
    let key = normalize_key_for_verify(key);

    let pub_bytes_vec = fs::read(pubkey_path).map_err(|e| format!("Không đọc public key: {e}"))?;
    let pub_bytes: [u8; 32] = pub_bytes_vec
        .try_into()
        .map_err(|_| "Public key phải đúng 32 bytes.".to_string())?;
    let verifying_key =
        VerifyingKey::from_bytes(&pub_bytes).map_err(|e| format!("Public key lỗi: {e}"))?;

    let key = key.trim();
    let parts: Vec<&str> = key.splitn(5, '|').collect();
    if parts.len() != 5 || parts[0] != "HHPOS" {
        return Err("Key sai định dạng. Cần: HHPOS|machine|expiry|edition|signature".to_string());
    }

    let payload = format!("{}|{}|{}|{}", parts[0], parts[1], parts[2], parts[3]);
    let sig_bytes = STANDARD
        .decode(parts[4])
        .map_err(|e| format!("Signature base64 lỗi: {e}"))?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| "Signature phải 64 bytes.".to_string())?;
    let signature = Signature::from_bytes(&sig_arr);

    verifying_key
        .verify(payload.as_bytes(), &signature)
        .map_err(|e| format!("Chữ ký không hợp lệ: {e}"))?;

    Ok(format!(
        "✅ Key hợp lệ\nMáy: {}\nGói: {}\nHết hạn: {}",
        parts[1],
        parts[3],
        expiry_to_display(parts[2].parse().unwrap_or(0))
    ))
}

struct LicenseGuiApp {
    tab: MainTab,
    registration_name: String,
    machine_id: String,
    edition_idx: usize,
    days_limit: i32,
    lifetime: bool,
    add_hyphens: bool,
    raw_license_key: String,
    display_license_key: String,
    private_key_path: String,
    public_key_path: String,
    output_dir: String,
    keygen_pub_bytes: String,
    license_log: Vec<LicenseLogEntry>,
    status_message: String,
    status_error: bool,
}

impl Default for LicenseGuiApp {
    fn default() -> Self {
        Self {
            tab: MainTab::KeysGenerator,
            registration_name: String::new(),
            machine_id: String::new(),
            edition_idx: 2,
            days_limit: 365,
            lifetime: false,
            add_hyphens: true,
            raw_license_key: String::new(),
            display_license_key: String::new(),
            private_key_path: "./keys/ed25519_private.key".to_string(),
            public_key_path: "./keys/ed25519_public.key".to_string(),
            output_dir: "./keys".to_string(),
            keygen_pub_bytes: String::new(),
            license_log: Vec::new(),
            status_message: "Sẵn sàng tạo key.".to_string(),
            status_error: false,
        }
    }
}

impl LicenseGuiApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        let mut app = Self::default();
        
        // Auto-generate key pair if not exists
        let keys_dir = "./keys";
        let priv_path = format!("{}/ed25519_private.key", keys_dir);
        let pub_path = format!("{}/ed25519_public.key", keys_dir);
        
        if !std::path::Path::new(&priv_path).exists() || !std::path::Path::new(&pub_path).exists() {
            if let Ok((p, q, _)) = create_key_pair(keys_dir) {
                app.private_key_path = p;
                app.public_key_path = q;
                app.status_message = "✓ Key pair tạo tự động. Sẵn sàng tạo key.".to_string();
                app.status_error = false;
            }
        } else {
            app.private_key_path = priv_path;
            app.public_key_path = pub_path;
        }
        
        let mut visuals = egui::Visuals::light();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(240, 240, 240);
        visuals.panel_fill = egui::Color32::from_rgb(236, 233, 216);
        visuals.window_fill = egui::Color32::from_rgb(236, 233, 216);
        cc.egui_ctx.set_visuals(visuals);
        app
    }

    fn set_ok(&mut self, msg: impl Into<String>) {
        self.status_error = false;
        self.status_message = msg.into();
    }

    fn set_err(&mut self, msg: impl Into<String>) {
        self.status_error = true;
        self.status_message = msg.into();
    }

    fn refresh_display_key(&mut self) {
        self.display_license_key =
            format_key_display(&self.raw_license_key, self.add_hyphens);
    }

    fn key_pair_status(&self) -> (String, egui::Color32) {
        let priv_path = std::path::Path::new(&self.private_key_path);
        let pub_path = std::path::Path::new(&self.public_key_path);
        if !priv_path.exists() {
            ("Missing private key".to_string(), egui::Color32::from_rgb(185, 28, 28))
        } else if !pub_path.exists() {
            ("Missing public key".to_string(), egui::Color32::from_rgb(185, 28, 28))
        } else {
            let valid_priv = fs::read(&self.private_key_path).map(|b| b.len() == 32).unwrap_or(false);
            let valid_pub = fs::read(&self.public_key_path).map(|b| b.len() == 32).unwrap_or(false);
            if !valid_priv || !valid_pub {
                ("Invalid key pair".to_string(), egui::Color32::from_rgb(180, 83, 9))
            } else {
                ("Key pair loaded".to_string(), egui::Color32::from_rgb(22, 163, 74))
            }
        }
    }

    fn effective_days(&self) -> i64 {
        if self.lifetime {
            -1
        } else {
            self.days_limit.max(1) as i64
        }
    }

    fn do_generate(&mut self) {
        let edition = EDITIONS[self.edition_idx];
        let days = self.effective_days();
        match generate_license_key(
            &self.machine_id,
            edition,
            days,
            &self.private_key_path,
        ) {
            Ok((key, expiry)) => {
                self.raw_license_key = key.clone();
                self.refresh_display_key();
                let expiry_str = expiry_to_display(expiry);
                self.license_log.insert(
                    0,
                    LicenseLogEntry {
                        time: format_local_time(),
                        customer_name: self.registration_name.clone(),
                        machine_id: self.machine_id.clone(),
                        edition: edition.to_string(),
                        expiry: expiry_str.clone(),
                        license_key: key,
                    },
                );
                self.set_ok(format!(
                    "Đã tạo key — Gói {edition}, hết hạn: {expiry_str}"
                ));
            }
            Err(e) => self.set_err(e),
        }
    }
}

impl App for LicenseGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("HangHoa POS — Keys Generator");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .selectable_label(self.tab == MainTab::KeysGenerator, "Tạo key")
                        .clicked()
                    {
                        self.tab = MainTab::KeysGenerator;
                    }
                    if ui
                        .selectable_label(self.tab == MainTab::Setup, "Setup / Key pair")
                        .clicked()
                    {
                        self.tab = MainTab::Setup;
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let color = if self.status_error {
                egui::Color32::from_rgb(185, 28, 28)
            } else {
                egui::Color32::from_rgb(22, 163, 74)
            };
            ui.colored_label(color, &self.status_message);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_max_width(640.0);
                ui.set_min_width(520.0);
                ui.add_space(12.0);
                match self.tab {
                    MainTab::KeysGenerator => self.ui_keys_generator(ui, ctx),
                    MainTab::Setup => self.ui_setup(ui),
                }
            });
        });
    }
}

impl LicenseGuiApp {
    fn ui_keys_generator(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut generate_now = false;

        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Tạo license mới").strong().size(18.0));
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Form tạo nhanh dành cho nhân viên kỹ thuật.")
                        .small()
                        .weak(),
                );
            });
            ui.add_space(12.0);

            ui.label(egui::RichText::new("Thông tin license").strong());
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.machine_id)
                    .hint_text("HWID / Mã máy")
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.registration_name)
                    .hint_text("Tên khách hàng")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Gói:");
                ui.add_space(8.0);
                egui::ComboBox::from_id_source("edition_main")
                    .selected_text(EDITIONS[self.edition_idx])
                    .show_ui(ui, |ui| {
                        for (i, e) in EDITIONS.iter().enumerate() {
                            ui.selectable_value(&mut self.edition_idx, i, *e);
                        }
                    });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.lifetime, "Vĩnh viễn");
                ui.add_space(16.0);
                ui.label("Thời hạn:");
                ui.add_space(8.0);
                if self.lifetime {
                    ui.label("—");
                } else {
                    ui.add_sized(
                        [90.0, 28.0],
                        egui::DragValue::new(&mut self.days_limit).range(1..=3650),
                    );
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("ngày").weak());
                }
            });

            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui
                    .add_sized(
                        [140.0, 38.0],
                        egui::Button::new("Tạo License")
                            .fill(egui::Color32::from_rgb(37, 99, 235)),
                    )
                    .clicked()
                {
                    generate_now = true;
                }
            });
        });

        ui.add_space(18.0);
        ui.group(|ui| {
            ui.label(egui::RichText::new("Kết quả").strong());
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.display_license_key)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY)
                    .interactive(false),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized([90.0, 32.0], egui::Button::new("Copy"))
                    .clicked()
                {
                    if self.raw_license_key.is_empty() {
                        self.set_err("Chưa có key để copy.");
                    } else {
                        ctx.copy_text(self.raw_license_key.clone());
                        self.set_ok("Đã copy key vào clipboard.");
                    }
                }
            });
        });

        if generate_now {
            self.machine_id = self.machine_id.trim().to_uppercase();
            self.do_generate();
        }
    }

    fn ui_license_log(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(
            egui::RichText::new("License Log — lịch sử key đã tạo trong phiên này")
                .strong(),
        );
        ui.add_space(8.0);

        if self.license_log.is_empty() {
            ui.label("Chưa có bản ghi. Tạo key ở tab Keys Generator.");
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("log_grid")
                .num_columns(6)
                .spacing([12.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Thời gian").strong());
                    ui.label(egui::RichText::new("Khách").strong());
                    ui.label(egui::RichText::new("Mã máy").strong());
                    ui.label(egui::RichText::new("Gói").strong());
                    ui.label(egui::RichText::new("Hết hạn").strong());
                    ui.label(egui::RichText::new("").strong());
                    ui.end_row();

                    let mut copy_idx = None;
                    for (i, entry) in self.license_log.iter().enumerate() {
                        ui.label(&entry.time);
                        ui.label(&entry.customer_name);
                        ui.label(&entry.machine_id);
                        ui.label(&entry.edition);
                        ui.label(&entry.expiry);
                        if ui.small_button("Copy").clicked() {
                            copy_idx = Some(i);
                        }
                        ui.end_row();
                    }
                    if let Some(i) = copy_idx {
                        ctx.copy_text(self.license_log[i].license_key.clone());
                        self.set_ok("Đã copy key từ log.");
                    }
                });
        });

        ui.add_space(12.0);
        if ui.button("Xóa log phiên này").clicked() {
            self.license_log.clear();
            self.set_ok("Đã xóa log.");
        }
    }

    fn ui_setup(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Thiết lập key pair").strong().size(18.0));
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Quản lý private/public key và thư mục lưu key.")
                        .small()
                        .weak(),
                );
            });

            ui.add_space(14.0);
            ui.label(egui::RichText::new("Folder lưu keys").strong());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.output_dir)
                        .desired_width(360.0),
                );
                ui.add_space(8.0);
                if ui.small_button("Browse").clicked() {
                    if let Some(p) = FileDialog::new().pick_folder() {
                        self.output_dir = p.to_string_lossy().to_string();
                    }
                }
            });

            ui.add_space(12.0);
            ui.label(egui::RichText::new("Đường dẫn key").strong());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Private key path").small().weak());
                    ui.add(
                        egui::TextEdit::singleline(&mut self.private_key_path)
                            .desired_width(f32::INFINITY),
                    );
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Public key path").small().weak());
                    ui.add(
                        egui::TextEdit::singleline(&mut self.public_key_path)
                            .desired_width(f32::INFINITY),
                    );
                });
            });

            ui.add_space(12.0);
            let (status_text, status_color) = self.key_pair_status();
            ui.horizontal(|ui| {
                ui.colored_label(status_color, status_text);
                if ui.button("Tạo key pair mới").clicked() {
                    match create_key_pair(&self.output_dir) {
                        Ok((priv_p, pub_p, bytes)) => {
                            self.private_key_path = priv_p;
                            self.public_key_path = pub_p;
                            self.keygen_pub_bytes = bytes;
                            self.set_ok("Tạo cặp khóa thành công.");
                        }
                        Err(e) => self.set_err(e),
                    }
                }
            });

            ui.add_space(12.0);
            egui::collapsing_header::CollapsingHeader::new("Nâng cao")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("Public key bytes (dùng khi cần nhúng vào app):");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.keygen_pub_bytes)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                });
        });
    }
}

fn main() {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 520.0])
            .with_min_inner_size([620.0, 480.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "HangHoa POS — Keys Generator",
        options,
        Box::new(|cc| Ok(Box::new(LicenseGuiApp::new(cc)))),
    );
}
