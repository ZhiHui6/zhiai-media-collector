@echo off
chcp 65001 >nul
cd /d "%~dp0"
if "%~1"=="" (
    set /p LINK=Link: 
) else (
    set "LINK=%~1"
)
if "%LINK%"=="" (
    echo No link provided.
    pause >nul
    exit /b 1
)
cargo run -p zhiai_core --example probe_page -- "%LINK%" > probe-output.txt 2>&1
echo.
type probe-output.txt
echo.
echo Output saved to %~dp0probe-output.txt
pause >nul