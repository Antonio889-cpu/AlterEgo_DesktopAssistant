# Alter Ego: installazione di Kokoro-82M (voce locale, offline dopo il primo download).
# Crea un ambiente Python dedicato in %APPDATA%\AlterEgo\kokoro\venv con PyTorch e la libreria
# "kokoro" (hexgrad, Apache-2.0) e scarica il modello (~330 MB) e le voci italiane/inglesi.
# Uso: lo avvia Alter Ego (Impostazioni → Voce → Installa Kokoro). Si può anche lanciare a mano:
#   powershell -ExecutionPolicy Bypass -File setup.ps1 [-Cuda]
param(
  [string]$Dir = (Join-Path $env:APPDATA "AlterEgo\kokoro"),
  [switch]$Cuda   # PyTorch con CUDA (scheda NVIDIA): più veloce, download molto più grande
)
$ErrorActionPreference = "Stop"
$Host.UI.RawUI.WindowTitle = "Alter Ego - installazione Kokoro"

function Run([string]$what) {
  # comandi esterni: $ErrorActionPreference non li controlla, serve $LASTEXITCODE
  & $args[0] @($args | Select-Object -Skip 1)
  if ($LASTEXITCODE -ne 0) { throw "$what non riuscito (codice $LASTEXITCODE)" }
}

function Find-Python {
  foreach ($v in "3.12", "3.11", "3.10") {
    try {
      & py "-$v" -c "import sys" 2>$null
      if ($LASTEXITCODE -eq 0) { return @("py", "-$v") }
    } catch {}
  }
  return $null
}

try {
  Write-Host "== Alter Ego: installazione di Kokoro-82M ==" -ForegroundColor Cyan
  Write-Host "Cartella: $Dir"
  New-Item -ItemType Directory -Force $Dir | Out-Null
  Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $Dir "installed.ok")

  $py = Find-Python
  if (-not $py) {
    Write-Host "Serve Python 3.10-3.12 (PyTorch non supporta ancora tutte le versioni piu' recenti)."
    Write-Host "Installo Python 3.12 con winget..." -ForegroundColor Yellow
    Run "winget" winget install -e --id Python.Python.3.12 --scope user --accept-package-agreements --accept-source-agreements
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
    $py = Find-Python
    if (-not $py) { throw "Python 3.12 installato ma non trovato: chiudi questa finestra e riprova." }
  }
  Write-Host "Python: $($py -join ' ')"

  $venv = Join-Path $Dir "venv"
  $vpy = Join-Path $venv "Scripts\python.exe"
  if (-not (Test-Path $vpy)) {
    Write-Host "Creo l'ambiente virtuale..."
    Run "venv" $py[0] $py[1] -m venv $venv
  }
  Run "pip" $vpy -m pip install --upgrade pip
  if ($Cuda) {
    Write-Host "Installo PyTorch (CUDA)..."
    Run "PyTorch" $vpy -m pip install torch --index-url https://download.pytorch.org/whl/cu124
  } else {
    Write-Host "Installo PyTorch (CPU)..."
    Run "PyTorch" $vpy -m pip install torch --index-url https://download.pytorch.org/whl/cpu
  }
  Write-Host "Installo kokoro..."
  Run "kokoro" $vpy -m pip install "kokoro>=0.9.4" numpy
  Write-Host "Scarico modello e voci (la prima volta ci vuole un po')..."
  Run "download del modello" $vpy (Join-Path $Dir "server.py") --warmup
  Set-Content -Path (Join-Path $Dir "installed.ok") -Value (Get-Date -Format o)
  Write-Host ""
  Write-Host "Fatto! Kokoro e' installato. In Alter Ego scegli la voce 'Kokoro (locale)'." -ForegroundColor Green
} catch {
  Write-Host ""
  Write-Host "ERRORE: $_" -ForegroundColor Red
  Write-Host "Puoi riprovare da Impostazioni -> Voce -> Installa Kokoro."
}
Read-Host "Premi Invio per chiudere"
