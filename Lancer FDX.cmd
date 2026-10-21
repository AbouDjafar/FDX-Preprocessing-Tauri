@echo off
cd /d "%~dp0"
if exist "dist\FDXAtelier.dist\FDXAtelier.exe" (
    start "" "dist\FDXAtelier.dist\FDXAtelier.exe"
) else (
    start "" ".venv\Scripts\pythonw.exe" "main.py"
)

