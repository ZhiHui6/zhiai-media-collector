@echo off
cd /d "%~dp0"
set RUST_BACKTRACE=1
start "zhiai-ui" cmd /c "npm run dev"
timeout /t 2 >nul
cargo run -p zhiai_media_collector
if errorlevel 1 (
    echo.
    echo Run failed. Press any key to close.
    pause >nul
    exit /b 1
)