@echo off
cd /d "%~dp0"
echo === cargo fmt --check ===
cargo fmt --check
if errorlevel 1 goto failed
echo === cargo check ===
cargo check --all-targets
if errorlevel 1 goto failed
echo === cargo clippy ===
cargo clippy --all-targets -- -D warnings
if errorlevel 1 goto failed
echo === cargo test ===
cargo test
if errorlevel 1 goto failed
echo === frontend typecheck + build ===
call npm run build
if errorlevel 1 goto failed
echo.
echo All checks passed.
exit /b 0

:failed
echo.
echo Checks failed. Press any key to close.
pause >nul
exit /b 1