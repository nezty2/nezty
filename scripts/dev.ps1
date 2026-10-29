# Start de app in ontwikkelmodus (hot reload voor de frontend).
# Gebruik: .\scripts\dev.ps1
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "env.ps1")
Set-Location (Join-Path $PSScriptRoot "..")
npm run tauri dev
