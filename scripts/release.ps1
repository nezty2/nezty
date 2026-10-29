# Een update uitbrengen in één keer: versie ophogen, bouwen, online zetten en aankondigen in Discord.
# Gebruik:  .\scripts\release.ps1 -Version 0.3.1 -Notes "Wat er nieuw is"
param([Parameter(Mandatory = $true)][string]$Version, [Parameter(Mandatory = $true)][string]$Notes, [switch]$Sign, [switch]$SkipDiscord, [switch]$NoPublish)
$ErrorActionPreference = "Stop"
$root = Join-Path $PSScriptRoot ".."

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Versie moet de vorm 1.2.3 hebben." }
$confPath = Join-Path $root "src-tauri\tauri.conf.json"
$current = (Get-Content $confPath -Raw | ConvertFrom-Json).version
$cv = [version]$current; $nv = [version]$Version
if ($nv -le $cv) { throw "Nieuwe versie $Version moet hoger zijn dan de huidige $current." }

# Versie in de drie projectbestanden bijwerken (alleen het versieveld).
$conf = Get-Content $confPath -Raw
$conf = $conf -replace '"version":\s*"[^"]+"', ('"version": "' + $Version + '"')
[IO.File]::WriteAllText($confPath, $conf, (New-Object Text.UTF8Encoding $false))
$pkgPath = Join-Path $root "package.json"
$pkg = Get-Content $pkgPath -Raw
$pkg = [regex]::Replace($pkg, '"version":\s*"[^"]+"', ('"version": "' + $Version + '"'), 1)
[IO.File]::WriteAllText($pkgPath, $pkg, (New-Object Text.UTF8Encoding $false))
$cargoPath = Join-Path $root "src-tauri\Cargo.toml"
$cargo = Get-Content $cargoPath -Raw
$cargo = [regex]::Replace($cargo, '(?m)^version = "[^"]+"', ('version = "' + $Version + '"'), 1)
[IO.File]::WriteAllText($cargoPath, $cargo, (New-Object Text.UTF8Encoding $false))
Write-Host "Versie: $current -> $Version"

& (Join-Path $PSScriptRoot "build.ps1") -Notes $Notes -Sign:$Sign
if ($NoPublish) { Write-Host "Gebouwd, nog niet gepubliceerd. Publiceer met: .\scripts\publish.ps1"; return }
& (Join-Path $PSScriptRoot "publish.ps1") -Notes $Notes -SkipDiscord:$SkipDiscord
