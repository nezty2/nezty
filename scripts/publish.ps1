# Zet de gebouwde update online: Supabase Storage (bucket "updates", waar de app zoekt) en ook als release
# "Latest" op GitHub. Daarna de tekst voor Discord.
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
# Expliciet UTF-8: Windows PowerShell leest een bestand zonder BOM anders als ANSI (dan wordt "één" onleesbaar).
$latest = Get-Content "$dir\latest.json" -Raw -Encoding UTF8 | ConvertFrom-Json
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

# Controleren dat het nieuwe bestand er staat. Met een unieke ?v= in de url, anders krijg je de cache van
# Supabase (die geeft de oude versie nog even). De app ziet de nieuwe versie zodra die cache verloopt.
$check = $null
for ($i = 0; $i -lt 12 -and -not $check; $i++) {
    $bust = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    try { $check = Invoke-RestMethod "$baseUrl/latest.json?v=$bust" -Headers @{ "Cache-Control" = "no-cache" } -TimeoutSec 20 } catch { $check = $null }
    if ($check -and $check.version -ne $version) { $check = $null }
    if (-not $check) { Start-Sleep -Seconds 5 }
}
if (-not $check) { throw "De bestanden zijn geüpload, maar $baseUrl/latest.json geeft nog niet versie $version. Is de bucket 'updates' openbaar?" }
Write-Host "Online: $baseUrl/latest.json (versie $version). Gebruikers zien hem binnen een paar minuten."

# Ook op GitHub: dezelfde ondertekende installer als release "Latest". Daar zoeken versies 0.3.x nog naar updates
# (zo springen die meteen naar de nieuwste), en de releasepagina blijft kloppen. Mislukt dit, dan staat de update
# al wel in Supabase: alleen een waarschuwing.
$repo = if ($publish.repo) { [string]$publish.repo } else { "nezty2/nezty" }
$gh = (Get-Command gh -ErrorAction SilentlyContinue).Source
if (-not $gh) { $gh = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages\GitHub.cli_Microsoft.Winget.Source_8wekyb3d8bbwe\bin\gh.exe" }
if (-not (Test-Path $gh)) {
    Write-Warning "gh (GitHub CLI) niet gevonden: de release staat alleen in Supabase, niet op GitHub."
} else {
    $tag = "v$version"
    $commit = (git rev-parse HEAD).Trim()
    $ghDir = Join-Path $env:TEMP "nezty-gh-$version"
    Remove-Item $ghDir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory $ghDir | Out-Null
    Copy-Item "$dir\$setup", "$dir\$setup.sig" $ghDir
    # latest.json voor GitHub: zelfde handtekening, maar de download wijst naar de GitHub-release.
    $ghLatest = [ordered]@{
        version   = $version
        notes     = $Notes
        pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        platforms = [ordered]@{ "windows-x86_64" = [ordered]@{ signature = (Get-Content "$dir\$setup.sig" -Raw).Trim(); url = "https://github.com/$repo/releases/download/$tag/$setup" } }
    }
    [IO.File]::WriteAllText("$ghDir\latest.json", ($ghLatest | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding $false))
    $ErrorActionPreference = "Continue"
    & $gh release view $tag --repo $repo 2>&1 | Out-Null
    $exists = $LASTEXITCODE -eq 0
    if ($exists) {
        Write-Warning "GitHub-release $tag bestaat al; niet overschreven."
    } else {
        & $gh release create $tag "$ghDir\$setup" "$ghDir\$setup.sig" "$ghDir\latest.json" --repo $repo --target $commit --title "nezty $version" --notes $Notes --latest 2>&1 | ForEach-Object { "$_" }
        if ($LASTEXITCODE -eq 0) { Write-Host "Ook op GitHub: https://github.com/$repo/releases/tag/$tag" } else { Write-Warning "GitHub-release maken mislukt (exitcode $LASTEXITCODE). De update staat wel in Supabase." }
    }
    $ErrorActionPreference = "Stop"
    Remove-Item $ghDir -Recurse -Force -ErrorAction SilentlyContinue
}

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
