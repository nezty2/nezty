# Zet de gebouwde update online (GitHub Releases) en kondigt hem aan in Discord.
# Gebruik:  .\scripts\publish.ps1 [-Notes "Wat er nieuw is"] [-SkipDiscord]
#
# Nodig (eenmalig):
#   - gh (GitHub CLI), ingelogd met:  gh auth login
#   - scripts\update.json  met baseUrl  https://github.com/<naam>/nezty/releases/latest/download
#   - scripts\publish.json met repo  "<naam>/nezty"  en eventueel discordWebhook
# Bouw eerst met .\scripts\build.ps1 (of gebruik .\scripts\release.ps1, dat beide doet).
param([string]$Notes = "", [switch]$SkipDiscord)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$gh = (Get-Command gh -ErrorAction SilentlyContinue).Source
if (-not $gh) { $gh = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\GitHub.cli_Microsoft.Winget.Source_8wekyb3d8bbwe\bin\gh.exe" }
if (-not (Test-Path $gh)) { throw "GitHub CLI (gh) niet gevonden. Installeer met: winget install --id GitHub.cli --scope user" }

$updateCfg = Join-Path $PSScriptRoot "update.json"
$publishCfg = Join-Path $PSScriptRoot "publish.json"
if (-not (Test-Path $updateCfg)) { throw "scripts\update.json ontbreekt (baseUrl)." }
if (-not (Test-Path $publishCfg)) { throw "scripts\publish.json ontbreekt (repo, discordWebhook)." }
$update = Get-Content $updateCfg -Raw | ConvertFrom-Json
$publish = Get-Content $publishCfg -Raw | ConvertFrom-Json
$baseUrl = ([string]$update.baseUrl).TrimEnd("/")
$repo = [string]$publish.repo
if (-not $repo) { throw "scripts\publish.json: repo ontbreekt, bijvoorbeeld ""naam/nezty""." }

$version = (Get-Content (Join-Path $PSScriptRoot "..\src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json).version
$dir = Join-Path (Get-Location) "release\update"
$setup = "nezty_${version}_x64-setup.exe"
$files = @("$dir\latest.json", "$dir\$setup", "$dir\$setup.sig")
foreach ($f in $files) { if (-not (Test-Path $f)) { throw "Ontbreekt: $f. Bouw eerst versie $version met .\scripts\build.ps1." } }
$latest = Get-Content "$dir\latest.json" -Raw | ConvertFrom-Json
if ($latest.version -ne $version) { throw "latest.json is voor versie $($latest.version), maar de app is $version. Bouw opnieuw." }
if (-not $Notes) { $Notes = [string]$latest.notes }
if (-not $Notes) { $Notes = "Nieuwe versie van nezty." }

# Controleren dat gh ingelogd is, vóór er iets online gaat.
$ErrorActionPreference = "Continue"
& $gh auth status 2>&1 | Out-Null
$authOk = $LASTEXITCODE -eq 0
$ErrorActionPreference = "Stop"
if (-not $authOk) { throw "gh is niet ingelogd. Voer eenmalig uit:  gh auth login" }

$tag = "v$version"
$ErrorActionPreference = "Continue"
& $gh release view $tag --repo $repo 2>&1 | Out-Null
$exists = $LASTEXITCODE -eq 0
$ErrorActionPreference = "Stop"
if ($exists) { throw "Versie $version staat al online ($tag). Verhoog het versienummer voor een nieuwe update." }

Write-Host "Online zetten: nezty $version naar github.com/$repo ..."
$ErrorActionPreference = "Continue"
& $gh release create $tag @files --repo $repo --title "nezty $version" --notes $Notes --latest 2>&1 | ForEach-Object { "$_" }
$code = $LASTEXITCODE
$ErrorActionPreference = "Stop"
if ($code -ne 0) { throw "GitHub-release maken mislukt (exitcode $code)." }

# Controleren dat de app de update kan vinden.
$check = $null
for ($i = 0; $i -lt 10 -and -not $check; $i++) {
    try { $check = Invoke-RestMethod "$baseUrl/latest.json" -Headers @{ "Cache-Control" = "no-cache" } -TimeoutSec 20 } catch { Start-Sleep -Seconds 3 }
}
if (-not $check -or $check.version -ne $version) { throw "De update staat online, maar $baseUrl/latest.json geeft nog niet versie $version. Controleer of de repository openbaar is." }
Write-Host "Online en bereikbaar: $baseUrl/latest.json (versie $version)"

# Aankondiging voor Discord: automatisch via een webhook als die is ingesteld, anders als tekst om zelf te plakken.
$download = "$baseUrl/$setup"
$content = "**nezty $version is uit!**`n$Notes`n`nHeb je nezty al? Open nezty en klik op **Nu bijwerken**.`nNog geen nezty? Download: $download"
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
Write-Host "Klaar. Gebruikers zien de update bij de volgende start van nezty."
