# Zet de licentieserver in één keer op. Draai dit op je eigen pc, nadat je "npx.cmd supabase login" hebt gedaan.
#
#   powershell -ExecutionPolicy Bypass -File server\setup.ps1 -ProjectRef <project-ref>
#
# Wat het doet:
#   1. Maakt de geheimen (alleen de eerste keer) in ..\nezty-licentie-geheimen\secrets.json, buiten de repo.
#   2. Koppelt de Supabase CLI aan je project, zet de geheimen op de server en uploadt de functie.
#   3. Zet NEZTY_LICENSE_URL en NEZTY_ADMIN_KEY als omgevingsvariabelen voor jouw Windows-gebruiker (voor de generator).
#   4. Maakt scripts\license.json (adres en publieke sleutel) zodat build.ps1 nezty met licentie bouwt.
#   5. Controleert of de server antwoordt.
# Geheimen worden nergens op het scherm getoond.
param([Parameter(Mandatory = $true)][string]$ProjectRef)
$ErrorActionPreference = "Stop"

$repo = Split-Path $PSScriptRoot -Parent
$secretDir = Join-Path (Split-Path $repo -Parent) "nezty-licentie-geheimen"
$secretFile = Join-Path $secretDir "secrets.json"

Write-Host "`n1. Geheimen" -ForegroundColor Cyan
if (Test-Path $secretFile) {
    Write-Host "   Bestaande geheimen gebruiken: $secretFile"
} else {
    New-Item -ItemType Directory -Force $secretDir | Out-Null
    Push-Location $repo
    & npm.cmd run -s licentie:sleutels-maken -- --bestand $secretFile | Out-Null
    Pop-Location
    if (-not (Test-Path $secretFile)) { throw "Geheimen maken mislukt." }
    Write-Host "   Nieuw gemaakt in $secretFile (maak hier een back-up van, net als van je updatesleutel)."
}
$s = Get-Content $secretFile -Raw | ConvertFrom-Json

Write-Host "`n2. Supabase" -ForegroundColor Cyan
Push-Location $PSScriptRoot
try {
    Write-Host "   Project koppelen. Vraagt hij om een databasewachtwoord: gewoon Enter (niet nodig)."
    & npx.cmd --yes supabase link --project-ref $ProjectRef
    if ($LASTEXITCODE -ne 0) { throw "Koppelen mislukt. Heb je 'npx.cmd supabase login' gedaan?" }
    Write-Host "   Geheimen op de server zetten..."
    & npx.cmd --yes supabase secrets set --project-ref $ProjectRef "LICENSE_SIGNING_KEY=$($s.LICENSE_SIGNING_KEY)" "LICENSE_KEY_PEPPER=$($s.LICENSE_KEY_PEPPER)" "ADMIN_KEY_SHA256=$($s.ADMIN_KEY_SHA256)" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Geheimen zetten mislukt." }
    Write-Host "   Functie uploaden..."
    & npx.cmd --yes supabase functions deploy license --no-verify-jwt --project-ref $ProjectRef
    if ($LASTEXITCODE -ne 0) { throw "Uploaden mislukt." }
} finally {
    Pop-Location
}

$url = "https://$ProjectRef.supabase.co/functions/v1/license"

Write-Host "`n3. Generator op deze pc" -ForegroundColor Cyan
[Environment]::SetEnvironmentVariable("NEZTY_LICENSE_URL", $url, "User")
[Environment]::SetEnvironmentVariable("NEZTY_ADMIN_KEY", [string]$s.NEZTY_ADMIN_KEY, "User")
$env:NEZTY_LICENSE_URL = $url
$env:NEZTY_ADMIN_KEY = [string]$s.NEZTY_ADMIN_KEY
Write-Host "   NEZTY_LICENSE_URL en NEZTY_ADMIN_KEY gezet (nieuwe vensters zien ze vanzelf)."

Write-Host "`n4. App koppelen" -ForegroundColor Cyan
$licenseJson = Join-Path $repo "scripts\license.json"
@{ url = $url; publicKey = [string]$s.NEZTY_LICENSE_PUBKEY } | ConvertTo-Json | Set-Content -Path $licenseJson -Encoding utf8
Write-Host "   scripts\license.json gemaakt (staat niet in de repo)."

Write-Host "`n5. Controle" -ForegroundColor Cyan
try {
    $r = Invoke-RestMethod -Method Post -Uri "$url/config" -Body "{}" -ContentType "application/json" -TimeoutSec 30
    if ($r.ok) { Write-Host "   De licentieserver antwoordt. Klaar!" -ForegroundColor Green }
    else { Write-Host "   De server antwoordt, maar met een fout: $($r.error)" -ForegroundColor Yellow }
} catch {
    Write-Host "   Geen antwoord van $url. Kijk in Supabase bij Edge Functions > license > Logs." -ForegroundColor Yellow
}

Write-Host "`nVolgende stap:  npm.cmd run licentie:instellingen -- --discord jouwnaam --email jij@voorbeeld.nl`n"
