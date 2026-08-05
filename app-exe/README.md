# HangHoa POS — Desktop App (Tauri)

Bọc toàn bộ giao diện web hiện có (`public/legacy/`) thành file `.exe` cài đặt được trên Windows, chạy hoàn toàn offline, mượt như phần mềm thật.

---

## Cây thư mục

```
app-exe/
├── package.json              ← scripts npm
├── README.md                 ← tài liệu này
└── src-tauri/
    ├── Cargo.toml            ← khai báo dependencies Rust
    ├── build.rs              ← build script Tauri
    ├── tauri.conf.json       ← cấu hình chính (window, bundle, icons)
    ├── capabilities/
    │   └── default.json      ← danh sách quyền native
    ├── icons/                ← đặt icons vào đây (xem mục "Đổi icon")
    └── src/
        ├── main.rs           ← entry point
        └── lib.rs            ← 10 native commands
```

Frontend: `../public/legacy/` (không thay đổi gì ở đây)

---

## Yêu cầu cài đặt

| Tool | Version | Link |
|------|---------|-------|
| Rust | latest stable | https://rustup.rs |
| Node.js | >= 18 | https://nodejs.org |
| WebView2 Runtime | có sẵn trên Win 10/11 | tự động cài khi build |

```powershell
# Cài Rust (chạy 1 lần duy nhất)
winget install Rustlang.Rustup
# hoặc tải installer tại https://rustup.rs
```

---

## Cách chạy Dev

```powershell
# 1. Vào thư mục app-exe
cd app-exe

# 2. Cài dependencies npm
npm install

# 3. Terminal 1: chạy dev server cho frontend
npm run dev

# 4. Terminal 2: chạy Tauri dev (mở cửa sổ desktop)
npm run tauri:dev
```

Khi dev, Tauri load frontend từ `http://localhost:1420` (dev server serve `public/legacy/`).

---

## Cách build .exe

```powershell
cd app-exe

# Cài dependencies (lần đầu)
npm install

# Build → tạo installer Windows
npm run tauri:build
```

**Output sẽ nằm tại:**
```
app-exe/src-tauri/target/release/bundle/
├── nsis/
│   └── HangHoa POS_1.0.0_x64-setup.exe   ← file cài đặt NSIS
└── msi/
    └── HangHoa POS_1.0.0_x64_en-US.msi   ← file MSI
```

> Build lần đầu mất 5–15 phút (Rust compile). Các lần sau nhanh hơn.

---

## Đổi icon app

### Cách nhanh nhất (khuyên dùng)

1. Chuẩn bị file `icon.png` kích thước **1024×1024px** (nền trong suốt, PNG)
2. Đặt vào `src-tauri/icons/source.png`
3. Chạy:

```powershell
npm run tauri:icon
```

Tauri tự tạo tất cả kích thước cần thiết (`32x32.png`, `128x128.png`, `icon.ico`, ...).

### Thủ công

Đặt các file sau vào `src-tauri/icons/`:
- `32x32.png`
- `128x128.png`
- `128x128@2x.png`
- `icon.ico` (256×256 bên trong)
- `icon.png` (256×256)

---

## Nơi lưu dữ liệu người dùng

Khi chạy app, dữ liệu được lưu tại:

```
C:\Users\<tên_user>\AppData\Roaming\com.hanghoa.pos\
├── pos_data.json           ← dữ liệu POS chính (sync từ IndexedDB)
└── backups/
    └── daily/
        └── backup_<ngày>.json   ← snapshot hàng ngày
```

> Gọi lệnh `get_app_data_dir` hoặc nhấn nút "Mở thư mục dữ liệu" trong app để mở nhanh.

---

## 10 Native Commands (dùng trong JS)

```javascript
import { invoke } from '@tauri-apps/api/core';

// 1. Đọc dữ liệu POS
const data = await invoke('read_pos_data');

// 2. Ghi dữ liệu POS
await invoke('write_pos_data', { content: JSON.stringify(data) });

// 3. Lấy đường dẫn thư mục dữ liệu
const dir = await invoke('get_app_data_dir');

// 4. Mở thư mục dữ liệu trong File Explorer
await invoke('open_data_dir');

// 5. Chọn thư mục backup (mở hộp thoại)
const backupDir = await invoke('pick_backup_dir');

// 6. Ghi file tùy ý
await invoke('write_local_file', { path: 'D:/backup/data.json', content: '{}' });

// 7. Đọc file tùy ý
const content = await invoke('read_local_file', { path: 'D:/backup/data.json' });

// 8. Lưu snapshot backup
const savedPath = await invoke('save_backup_file', {
  content: JSON.stringify(data),
  label: '2026-04-14'   // để null để dùng timestamp tự động
});

// 9. Liệt kê các backup
const backups = await invoke('list_backups');  // trả về mảng đường dẫn

// 10. Khởi động lại app
await invoke('restart_app');
```

> Lưu ý: Tauri API chỉ hoạt động khi chạy trong Tauri, không hoạt động trong trình duyệt thường.
> Dùng `window.__TAURI__` để kiểm tra môi trường trước khi gọi.

---

## Cấu hình cửa sổ

| Thuộc tính | Giá trị |
|-----------|---------|
| Kích thước mặc định | 1600 × 900 |
| Tối thiểu | 1280 × 720 |
| Tối đa | 1920 × 1080 |
| Căn giữa màn hình | ✓ |
| Nhớ kích thước lần cuối | ✓ (plugin window-state) |
| Nhớ vị trí lần cuối | ✓ (plugin window-state) |
| Hardware acceleration | ✓ (WebView2 mặc định) |

---

## Release bản mới

```powershell
# 1. Tăng version trong tauri.conf.json và package.json

# 2. Build
npm run tauri:build

# 3. Chia sẻ file installer
#    src-tauri/target/release/bundle/nsis/HangHoa POS_X.X.X_x64-setup.exe
```

---

## Ghi chú kỹ thuật

- Frontend load từ **thư mục local** (`public/legacy/`) — không qua mạng, không cần server
- WebView2 (Chromium-based) đã bật hardware acceleration mặc định → scroll và animation mượt
- `visible: false` trong config → cửa sổ ẩn cho đến khi WebView tải xong → không bị nháy trắng khi mở
- `tauri-plugin-window-state` tự động lưu/khôi phục `size` và `position` mỗi lần đóng/mở
- Build release dùng LTO + strip → file exe nhỏ nhất có thể
