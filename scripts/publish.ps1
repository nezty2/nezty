# Zet de gebouwde update online (Supabase Storage, bucket "updates") en kondigt hem aan in Discord.
# Gebruik:  .\scripts\publish.ps1 [-Notes "Wat er nieuw is"] [-SkipDiscord]
#
# Nodig (eenmalig):
#   - scripts\update.json  met baseUrl  https://<project-ref>.supabase.co/storage/v1/object/public/updates
#   - scripts\publish.json met projectRef, serviceRoleKey (Supabase > Project Settings > API) en eventueel discordWebhook
# Bouw eerst met .\scripts\build.ps1 (of gebruik .\scripts\release.ps1, dat beide doet).
# Normaal gesproken hoef je dit niet zelf te doen: een push naar main laat GitHub Actions bouwen en publiceren.
param([string]$Notes = "", [switch]$SkipDiscord)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$updateCfg = Join-Path $PSScriptRoot "update.json"
$publishCfg = Join-Path $PSScriptRoot "publish.json"
if (-not (Test-Path $updateCfg)) { throw "scripts\update.json ontbreekt (baseUrl)." }
if (-not (Test-Path $publishCfg)) { throw "scripts\publish.json ontbreekt (projectRef, serviceRoleKey, discordWebhook)." }
$update = Get-Content $updateCfg -Raw | ConvertFrom-Json
$publish = Get-Content $publishCfg -Raw | ConvertFrom-Json
$baseUrl = ([string]$update.baseUrl).TrimEnd("/")
$ref = [string]$publish.projectRef
$key = [string]$publish.serviceRoleKey
if (-not $ref -or -not $key) { throw "scripts\publish.json: projectRef of serviceRoleKey ontbreekt." }

$version = (Get-Content (Join-Path $PSScriptRoot "..\src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json).version
if (Test-Path (Join-Path (Get-Location) "release\SELFTEST")) { throw "release bevat een testbuild met zelftests (build.ps1 -Selftest). Bouw eerst opnieuw zonder -Selftest." }
$dir = Join-Path (Get-Location) "release\update"
$setup = "Nezty-Tweaks-Setup-${version}.exe"
$files = @("$dir\$setup", "$dir\$setup.sig", "$dir\latest.json")
foreach ($f in $files) { if (-not (Test-Path $f)) { throw "Ontbreekt: $f. Bouw eerst versie $version met .\scripts\build.ps1." } }
$latest = Get-Content "$dir\latest.json" -Raw | ConvertFrom-Json
if ($latest.version -ne $version) { throw "latest.json is voor versie $($latest.version), maar de app is $version. Bouw opnieuw." }
if (-not $Notes) { $Notes = [string]$latest.notes }
if (-not $Notes) { $Notes = "Nieuwe versie van nezty." }

# Staat deze versie al online? Dan niets overschrijven.
try {
    $online = Invoke-RestMethod "$baseUrl/latest.json" -Headers @{ "Cache-Control" = "no-cache" } -TimeoutSec 20
    if ([version]$online.version -ge [version]$version) { throw "Versie $($online.version) staat al online. Verhoog het versienummer voor een nieuwe update." }
} catch [System.Net.WebException] {
    # Nog geen latest.json: eerste keer publiceren.
}

& (Join-Path $PSScriptRoot "upload-supabase.ps1") -ProjectRef $ref -ServiceRoleKey $key -Files $files

# Controleren dat de app de update kan vinden.
$check = $null
for ($i = 0; $i -lt 10 -and -not $check; $i++) {
    try { $check = Invoke-RestMethod "$baseUrl/latest.json" -Headers @{ "Cache-Control" = "no-cache" } -TimeoutSec 20 } catch { Start-Sleep -Seconds 3 }
    if ($check -and $check.version -ne $version) { $check = $null; Start-Sleep -Seconds 3 }
}
if (-not $check) { throw "De bestanden zijn geüpload, maar $baseUrl/latest.json geeft nog niet versie $version. Is de bucket 'updates' openbaar?" }
Write-Host "Online en bereikbaar: $baseUrl/latest.json (versie $version)"

# Aankondiging voor Discord: automatisch via een webhook als die is ingesteld, anders als tekst om zelf te plakken.
$download = "$baseUrl/$setup"
$content = "**nezty $version is uit!**`n$Notes`n`nHeb je nezty al? Dan wordt hij vanzelf bijgewerkt.`nNog geen nezty? Download: $download"
if ($content.Length -gt 1900) { $content = $content.Substring(0, 1900) }
if (-not $SkipDiscord -and $publish.discordWebhook) {
    $body = @{ content = $content; username = "nezty" } | ConvertTo-Json
    Invoke-RestMethod -Method Post -Uri ([string]$publish.discordWebhook) -ContentType "application/json; charset=utf-8" -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
    Write-Host "Aangekondigd in Discord."
} else {
    Write-Host ""
    Write-Host "Tekst voor je Discord-bericht:"
    Write-Host "------------------------------"
    Write-Host $content
    Write-Host "------------------------------"
}
