// Ẩn cửa sổ console trên Windows release build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    hangho_pos_desktop::run()
}
