@echo off
cd /d "%~dp0"
echo Building license-generator...
cargo build --release
if errorlevel 1 (
    echo.
    echo Build failed.
    pause
    exit /b 1
)
echo.
echo Build succeeded.
echo Output: %cd%\target\release\license-generator.exe
pause
