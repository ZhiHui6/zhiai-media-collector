@echo off
cd /d "%~dp0"
call npm run build
if errorlevel 1 goto failed
cargo build -p zhiai_media_collector --release --features custom-protocol
if errorlevel 1 goto failed
if not exist release mkdir release
copy /y "target\release\zhiai_media_collector.exe" "release\zhiai_media_collector.exe" >nul
if errorlevel 1 goto failed
echo.
echo Built: %~dp0release\zhiai_media_collector.exe
exit /b 0

:failed
echo.
echo Build failed. Press any key to close.
pause >nul
exit /b 1