use sha2::{Digest, Sha256};

/// Read a string value from the Windows Registry (HKEY_LOCAL_MACHINE).
/// Returns empty string on any error (key not found, access denied, non-Windows, etc.).
#[cfg(target_os = "windows")]
fn read_reg_str(path: &str, name: &str) -> String {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(path)
        .and_then(|k| k.get_value::<String, _>(name))
        .unwrap_or_default()
        .trim()
        .replace('\0', "")
        .to_string()
}

#[cfg(not(target_os = "windows"))]
fn read_reg_str(_path: &str, _name: &str) -> String {
    String::new()
}

/// Collect a stable hardware fingerprint for this machine.
///
/// Sources (Windows Registry — no wmic, no admin required):
/// 1. MachineGuid     — most reliable, set once at OS install
/// 2. BaseBoardSerialNumber — motherboard serial
/// 3. SystemManufacturer    — OEM name (stabilises hash when serial is missing)
/// 4. ProcessorNameString   — CPU model string
/// 5. COMPUTERNAME env var  — hostname fallback
///
/// Returns a 14-char string in XXXX-XXXX-XXXX format (first 6 bytes of SHA-256).
pub fn collect_machine_id() -> String {
    let machine_guid = read_reg_str(
        "SOFTWARE\\Microsoft\\Cryptography",
        "MachineGuid",
    );
    let board_serial = read_reg_str(
        "HARDWARE\\DESCRIPTION\\System\\BIOS",
        "BaseBoardSerialNumber",
    );
    let sys_manuf = read_reg_str(
        "HARDWARE\\DESCRIPTION\\System\\BIOS",
        "SystemManufacturer",
    );
    let cpu_name = read_reg_str(
        "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",
        "ProcessorNameString",
    );
    let hostname = std::env::var("COMPUTERNAME").unwrap_or_default();

    let raw = format!("{machine_guid}|{board_serial}|{sys_manuf}|{cpu_name}|{hostname}");

    let mut h = Sha256::new();
    // Domain-separated prefix prevents collision with other uses of raw data
    h.update(b"HHPOS_MID_v1:");
    h.update(raw.as_bytes());
    let digest = h.finalize();

    // First 6 bytes → 12 hex uppercase chars → XXXX-XXXX-XXXX (14 chars total)
    let hex: String = digest[..6].iter().map(|b| format!("{b:02X}")).collect();
    format!("{}-{}-{}", &hex[..4], &hex[4..8], &hex[8..12])
}
