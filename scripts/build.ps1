# Maakt de release-build, de NSIS-installer en de updatebestanden.
# Gebruik:  .\scripts\build.ps1                         (zonder Authenticode-ondertekening)
#           .\scripts\build.ps1 -Sign                   (onderteken exe en installer, vereist scripts\signing.json)
#           .\scripts\build.ps1 -Notes "Wat is er nieuw" (tekst die gebruikers bij de update zien)
#           .\scripts\build.ps1 -UpdateConfig pad.json   (andere updatebron, bijvoorbeeld om te testen)
#
# Updates: elke build maakt ondertekende updatebestanden met de sleutel uit nezty-update-sleutel.
# Staat er in scripts\update.json een "baseUrl", dan wordt die als updatebron ingebouwd en komt in
# release\update alles wat online moet: de installer, de handtekening (.sig) en latest.json.
#           .\scripts\build.ps1 -Version 0.2.99         (andere versie dan in tauri.conf.json, voor tests)
param([switch]$Sign, [string]$Notes = "", [string]$UpdateConfig = "", [string]$Version = "")
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "env.ps1")
Set-Location (Join-Path $PSScriptRoot "..")

$version = if ($Version) { $Version } else { (Get-Content (Join-Path $PSScriptRoot "..\src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json).version }

# ---------- Updatebron en -sleutel ----------
if (-not $UpdateConfig) { $UpdateConfig = Join-Path $PSScriptRoot "update.json" }
$update = $null
if (Test-Path $UpdateConfig) { $update = Get-Content $UpdateConfig -Raw | ConvertFrom-Json }
$baseUrl = if ($update -and $update.baseUrl) { ([string]$update.baseUrl).TrimEnd("/") } else { "" }
if (-not $Notes -and $update -and $update.notes) { $Notes = [string]$update.notes }

$keyPath = if ($update -and $update.keyPath) { [string]$update.keyPath } else { Join-Path $PSScriptRoot "..\..\nezty-update-sleutel\nezty.key" }
if (-not (Test-Path $keyPath)) {
    throw "Updatesleutel niet gevonden: $keyPath. Zonder deze sleutel kunnen bestaande gebruikers geen updates krijgen. Zet de sleutel terug uit je back-up."
}
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $keyPath -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

$override = @{}
if ($Version) { $override.version = $Version }
if ($baseUrl) {
    $updater = @{ endpoints = @("$baseUrl/latest.json") }
    # Alleen voor een lokale test: http toestaan. Echte updates gaan altijd via https.
    if ($update.allowInsecure -eq $true) { $updater.allowInsecure = $true }
    $override.plugins = @{ "nezty-update" = $updater }
    Write-Host "Updatebron: $baseUrl/latest.json"
} else {
    Write-Warning "Geen updatebron ingesteld (scripts\update.json met baseUrl). Deze build zoekt niet naar updates."
}

if ($Sign) {
    if (-not (Test-Path (Join-Path $PSScriptRoot "signing.json"))) {
        throw "scripts\signing.json ontbreekt. Zie ONDERTEKENEN.md."
    }
    $signScript = (Resolve-Path (Join-Path $PSScriptRoot "sign.ps1")).Path
    $override.bundle = @{
        windows = @{
            signCommand = @{
                cmd  = "powershell.exe"
                args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $signScript, "%1")
            }
        }
    }
    Write-Host "Ondertekenen staat aan."
}

$extra = @()
if ($override.Count -gt 0) {
    $overridePath = Join-Path $env:TEMP "nezty-build.conf.json"
    $override | ConvertTo-Json -Depth 8 | Set-Content -Path $overridePath -Encoding UTF8
    $extra = @("--config", $overridePath)
}

# npm en cargo schrijven voortgang naar stderr; dat is geen fout. Alleen de exitcode telt.
$ErrorActionPreference = "Continue"
npm run tauri build -- @extra 2>&1 | ForEach-Object { "$_" }
$code = $LASTEXITCODE
$ErrorActionPreference = "Stop"
if ($code -ne 0) { throw "Build mislukt (exitcode $code)." }

# ---------- release-map ----------
$release = Join-Path (Get-Location) "release"
New-Item -ItemType Directory -Force $release | Out-Null
$setupName = "nezty_${version}_x64-setup.exe"
$bundle = "$env:CARGO_TARGET_DIR\release\bundle\nsis"
# Alleen de installer van de huidige versie: oude installers weghalen zodat er geen verwarring is.
Get-ChildItem $release -Filter "nezty_*_x64-setup.exe" | Where-Object { $_.Name -ne $setupName } | Remove-Item -Force
Copy-Item "$bundle\$setupName" $release -Force
$exeCopied = $true
try {
    Copy-Item "$env:CARGO_TARGET_DIR\release\nezty.exe" $release -Force -ErrorAction Stop
} catch {
    # De losse exe is in gebruik (nezty draait vanuit deze map). De installer is wel bijgewerkt.
    $exeCopied = $false
    Write-Warning "release\nezty.exe is in gebruik en is niet vervangen. Sluit nezty en bouw opnieuw, of gebruik de installer."
}
# Met de GNU-toolchain heeft de losse exe WebView2Loader.dll naast zich nodig (de installer zet die er zelf bij).
$loader = "$env:CARGO_TARGET_DIR\release\WebView2Loader.dll"
if ((Test-Path $loader) -and -not (Test-Path (Join-Path $release "WebView2Loader.dll"))) { Copy-Item $loader $release -Force }
if ($Sign -and $exeCopied) {
    # Tauri ondertekent de exe in de installer, maar zet de losse exe na afloop terug naar de ongetekende versie.
    & (Join-Path $PSScriptRoot "sign.ps1") -File (Join-Path $release "nezty.exe")
}

# ---------- updatebestanden ----------
# Updatehandtekening (minisign) over de definitieve installer, dus na een eventuele Authenticode-ondertekening.
$sigFile = "$bundle\$setupName.sig"
if (Test-Path $sigFile) { Remove-Item $sigFile -Force }
$ErrorActionPreference = "Continue"
npx tauri signer sign "$bundle\$setupName" 2>&1 | ForEach-Object { "$_" }
$ErrorActionPreference = "Stop"
if (-not (Test-Path $sigFile)) { throw "Updatehandtekening maken mislukt: $sigFile ontbreekt." }
$updateDir = Join-Path $release "update"
if (Test-Path $updateDir) { Remove-Item $updateDir -Recurse -Force }
New-Item -ItemType Directory $updateDir | Out-Null
Copy-Item "$bundle\$setupName", $sigFile $updateDir
if ($baseUrl) {
    $latest = [ordered]@{
        version   = $version
        notes     = $Notes
        pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        platforms = [ordered]@{
            "windows-x86_64" = [ordered]@{
                signature = (Get-Content $sigFile -Raw).Trim()
                url       = "$baseUrl/$setupName"
            }
        }
    }
    # Zonder BOM: de updater leest gewone JSON.
    [IO.File]::WriteAllText((Join-Path $updateDir "latest.json"), ($latest | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding $false))
}

Write-Host ""
Write-Host "Klaar. Bestanden staan in: $release"
if ($baseUrl) {
    Write-Host "Zet voor de update deze bestanden online op $baseUrl :"
    Get-ChildItem $updateDir | ForEach-Object { Write-Host "  $($_.Name)" }
}
if ($Sign) {
    Get-ChildItem $release -Filter *.exe | ForEach-Object {
        $s = Get-AuthenticodeSignature $_.FullName
        Write-Host ("  {0}: {1} ({2})" -f $_.Name, $s.Status, $s.SignerCertificate.Subject)
    }
}
