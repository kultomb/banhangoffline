use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Helpers ────────────────────────────────────────────────────────────────

fn app_data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn ensure_dir(path: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("Không tạo được thư mục: {e}"))
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ─── 1. Đọc dữ liệu POS chính ───────────────────────────────────────────────

#[tauri::command]
fn read_pos_data(app: AppHandle) -> Result<String, String> {
    let path = app_data_dir(&app).join("pos_data.json");
    match fs::read_to_string(&path) {
        Ok(content) => Ok(content),
        Err(_) => Ok(String::from("null")),
    }
}

// ─── 2. Ghi dữ liệu POS chính ───────────────────────────────────────────────

#[tauri::command]
fn write_pos_data(app: AppHandle, content: String) -> Result<(), String> {
    let dir = app_data_dir(&app);
    ensure_dir(&dir)?;
    fs::write(dir.join("pos_data.json"), content)
        .map_err(|e| format!("Lỗi ghi file: {e}"))
}

// ─── 3. Lấy đường dẫn thư mục dữ liệu ─────────────────────────────────────

#[tauri::command]
fn get_app_data_dir(app: AppHandle) -> String {
    app_data_dir(&app).to_string_lossy().to_string()
}

// ─── 4. Mở thư mục dữ liệu bằng File Explorer ──────────────────────────────

#[tauri::command]
fn open_data_dir(app: AppHandle) -> Result<(), String> {
    let dir = app_data_dir(&app);
    ensure_dir(&dir)?;
    let path_str = dir.to_string_lossy().to_string();
    app.opener()
        .open_url(&path_str, None::<&str>)
        .map_err(|e| format!("Không mở được thư mục: {e}"))
}

// ─── 5. Chọn thư mục backup (trả về đường dẫn) ─────────────────────────────

#[tauri::command]
fn pick_backup_dir(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let result = app.dialog()
        .file()
        .set_title("Chọn thư mục lưu backup")
        .blocking_pick_folder();
    match result {
        Some(path) => Ok(path.to_string()),
        None => Err(String::from("Người dùng huỷ")),
    }
}

// ─── 6. Ghi file tùy ý ra đĩa ──────────────────────────────────────────────

#[tauri::command]
fn write_local_file(path: String, content: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        ensure_dir(&parent.to_path_buf())?;
    }
    fs::write(&p, content).map_err(|e| format!("Lỗi ghi {path}: {e}"))
}

// ─── 7. Đọc file tùy ý từ đĩa ──────────────────────────────────────────────

#[tauri::command]
fn read_local_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("Lỗi đọc {path}: {e}"))
}

// ─── 8. Lưu snapshot backup có timestamp ────────────────────────────────────

#[tauri::command]
fn save_backup_file(app: AppHandle, content: String, label: Option<String>) -> Result<String, String> {
    let dir = app_data_dir(&app).join("backups").join("daily");
    ensure_dir(&dir)?;
    let ts = unix_ts();
    let name = match label {
        Some(l) if !l.is_empty() => format!("backup_{}.json", l),
        _ => format!("backup_{}.json", ts),
    };
    let path = dir.join(&name);
    fs::write(&path, content).map_err(|e| format!("Lỗi ghi backup: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

// ─── 9. Liệt kê các file backup ────────────────────────────────────────────

#[tauri::command]
fn list_backups(app: AppHandle) -> Result<Vec<String>, String> {
    let dir = app_data_dir(&app).join("backups").join("daily");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut files: Vec<String> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    files.sort();
    files.reverse();
    Ok(files)
}

// ─── 10. Khởi động lại ứng dụng ────────────────────────────────────────────

#[tauri::command]
fn restart_app(app: AppHandle) {
    tauri::process::restart(&app.env());
}



// ─── App Entry ──────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Plugin nhớ kích thước & vị trí cửa sổ giữa các lần mở
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Hiện cửa sổ sau khi WebView đã tải xong → không bị nháy trắng
            let window = app.get_webview_window("main").unwrap();
            window.show().unwrap();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_pos_data,
            write_pos_data,
            get_app_data_dir,
            open_data_dir,
            pick_backup_dir,
            write_local_file,
            read_local_file,
            save_backup_file,
            list_backups,
            restart_app,
        ])
        .run(tauri::generate_context!())
        .expect("Lỗi khởi động HangHoa POS");
}
