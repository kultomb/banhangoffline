@echo off
setlocal enabledelayedexpansion
title HangHoa POS - Build Desktop App

echo.
echo ================================================
echo   HangHoa POS - Build Windows .exe (Tauri)
echo ================================================
echo.

:: -- Kiem tra Rust --
echo [1/4] Kiem tra Rust...
rustc --version >nul 2>&1
if %errorlevel% neq 0 (
    echo.
    echo   [LOI] Chua cai Rust!
    echo.
    echo   Cai Rust tai: https://rustup.rs
    echo   Hoac chay:    winget install Rustlang.Rustup
    echo.
    echo   Sau khi cai xong, mo lai terminal va chay lai file nay.
    echo.
    pause
    exit /b 1
)
for /f "tokens=*" %%v in ('rustc --version') do echo   OK: %%v

:: -- Kiem tra Node.js --
echo.
echo [2/4] Kiem tra Node.js...
where node >nul 2>&1
if %errorlevel% neq 0 (
    echo.
    echo   [LOI] Chua cai Node.js!
    echo   Tai tai: https://nodejs.org
    echo   Chon ban LTS de cai.
    echo.
    pause
    exit /b 1
)
for /f "tokens=*" %%v in ('node --version 2^>^&1') do echo   OK: Node.js %%v

:: -- Cai npm dependencies --
echo.
echo [3/4] Cai npm dependencies...
if not exist node_modules (
    echo   Dang cai @tauri-apps/cli va serve...
    call npm install
    if %errorlevel% neq 0 (
        echo   [LOI] npm install that bai!
        pause
        exit /b 1
    )
    echo   OK: node_modules da san sang
) else (
    echo   OK: node_modules da co san, bo qua
)

:: -- Build --
echo.
echo [4/4] Dang build phien ban moi... (Tu dong tang Version & Luu rieng vao thu muc dist)
echo       Dung dong cua so nay!
echo.
call npm run tauri:build
if %errorlevel% neq 0 (
    echo.
    echo ================================================
    echo   [THAT BAI] Build loi, xem thong bao o tren
    echo ================================================
    echo.
    echo   Nguyen nhan pho bien:
    echo   - Chua cai Rust: chay "rustup update" roi thu lai
    echo   - Thieu icon: dat file PNG 1024x1024 vao src-tauri\icons\source.png
    echo     roi chay: npm run tauri:icon
    echo.
    pause
    exit /b 1
)

:: -- Hien thi ket qua --
echo.
echo ================================================
echo   BUILD THANH CONG! CAC BAN BUILD LUON DUOC LUU RIENG KHONG DE!
echo ================================================
echo.
echo   File installer moi da duoc luu tai thu muc DIST:
echo.

set "DIST_DIR=%~dp0dist"

if exist "%DIST_DIR%\" (
    for /f "tokens=*" %%f in ('dir /b "%DIST_DIR%\*.exe" 2^>nul') do (
        echo   [EXE]  %DIST_DIR%\%%f
        set "NSIS_FILE=%DIST_DIR%\%%f"
    )
)

echo.
set /p OPEN_DIR="  Mo thu muc DIST chua tat ca cac phien ban build? (Y/N): "
if /i "%OPEN_DIR%"=="Y" (
    if exist "%DIST_DIR%\" (
        explorer "%DIST_DIR%"
    )
)

if defined NSIS_FILE (
    echo.
    set /p RUN_INSTALL="  Chay installer cai dat luon? (Y/N): "
    if /i "!RUN_INSTALL!"=="Y" (
        start "" "!NSIS_FILE!"
    )
)

echo.
pause
