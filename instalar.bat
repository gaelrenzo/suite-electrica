@echo off
rem Instalador de Suite Electrica para Windows 10/11.
rem Crea un entorno .venv en esta carpeta, instala la suite y agrega bin\ al PATH del usuario.
setlocal
cd /d "%~dp0"

where py >nul 2>nul
if errorlevel 1 (
  echo No se encontro Python. Instale Python 3.11 o superior desde https://www.python.org/downloads/
  echo y marque la opcion "Add python.exe to PATH". Luego ejecute de nuevo este archivo.
  pause
  exit /b 1
)
py -3 -c "import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)"
if errorlevel 1 (
  echo Se requiere Python 3.11 o superior. Version encontrada:
  py -3 --version
  pause
  exit /b 1
)

if not exist .venv (
  echo Creando entorno .venv ...
  py -3 -m venv .venv
  if errorlevel 1 goto :error
)
echo Instalando dependencias, puede tardar unos minutos ...
.venv\Scripts\python -m pip install --upgrade pip
.venv\Scripts\python -m pip install -e ".[dev,gui]"
if errorlevel 1 goto :error

echo Ejecutando pruebas ...
.venv\Scripts\python -m pytest -q
if errorlevel 1 echo ADVERTENCIA: algunas pruebas fallaron; revise el mensaje anterior.

powershell -NoProfile -ExecutionPolicy Bypass -Command "$d = (Resolve-Path 'bin').Path; $p = [Environment]::GetEnvironmentVariable('Path', 'User'); if (-not $p) { $p = '' }; if (($p -split ';') -notcontains $d) { [Environment]::SetEnvironmentVariable('Path', (($p.TrimEnd(';') + ';' + $d).TrimStart(';')), 'User'); Write-Host 'Carpeta bin agregada al PATH del usuario.' } else { Write-Host 'bin ya estaba en el PATH.' }"

powershell -NoProfile -ExecutionPolicy Bypass -Command "$exe = (Resolve-Path '.venv\Scripts\suite-electrica.exe').Path; $sh = New-Object -ComObject WScript.Shell; foreach ($dir in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) { $l = $sh.CreateShortcut((Join-Path $dir 'Suite Electrica.lnk')); $l.TargetPath = $exe; $l.WorkingDirectory = (Get-Location).Path; $l.Description = 'TopoCraft y LineCraft'; $l.Save() }; Write-Host 'Accesos directos creados en el Escritorio y el menu Inicio.'"

echo.
echo Listo. Abra "Suite Electrica" desde el Escritorio o el menu Inicio.
echo Comandos (en una NUEVA terminal):
echo    topocraft --help
echo    linecraft --help
echo    gridcraft --help
echo Guia: docs\GUIA_RAPIDA.md
pause
exit /b 0

:error
echo La instalacion fallo. Revise los mensajes anteriores.
pause
exit /b 1
