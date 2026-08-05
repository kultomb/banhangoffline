use tauri::Manager;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn app_data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Đọc dữ liệu POS từ file cục bộ
#[tauri::command]
fn read_pos_data(app: tauri::AppHandle) -> Result<String, String> {
    let path = app_data_dir(&app).join("pos_data.json");
    match fs::read_to_string(&path) {
        Ok(content) => Ok(content),
        Err(_) => Ok(String::from("null")),
    }
}

/// Ghi dữ liệu POS vào file cục bộ
#[tauri::command]
fn write_pos_data(app: tauri::AppHandle, content: String) -> Result<(), String> {
    let dir = app_data_dir(&app);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("pos_data.json");
    fs::write(&path, content).map_err(|e| e.to_string())
}

/// Lấy đường dẫn thư mục backup
#[tauri::command]
fn get_backup_dir(app: tauri::AppHandle) -> String {
    app_data_dir(&app)
        .join("backups")
        .to_string_lossy()
        .to_string()
}

/// Lưu backup với tên file theo timestamp
#[tauri::command]
fn save_backup_file(app: tauri::AppHandle, content: String) -> Result<String, String> {
    let dir = app_data_dir(&app).join("backups");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let filename = format!("backup_{}.json", ts);
    let path = dir.join(&filename);
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Liệt kê các file backup
#[tauri::command]
fn list_backups(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let dir = app_data_dir(&app).join("backups");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    files.sort();
    files.reverse();
    Ok(files)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            read_pos_data,
            write_pos_data,
            get_backup_dir,
            save_backup_file,
            list_backups,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
