# Zet de build-omgeving klaar voor de GNU-toolchain zonder Visual Studio Build Tools.
# Gebruik: . .\scripts\env.ps1   (met de punt, zodat PATH in je sessie blijft)

$winlibs = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"

if (Test-Path $winlibs) {
    $env:PATH = "$winlibs;$cargoBin;$env:PATH"
    Write-Host "MinGW-w64 (WinLibs) en cargo staan op het PATH."
} else {
    $env:PATH = "$cargoBin;$env:PATH"
    Write-Host "WinLibs niet gevonden. Met Visual Studio Build Tools (MSVC) is dit niet nodig; anders: winget install BrechtSanders.WinLibs.POSIX.UCRT --scope user"
}

# Korte doelmap voorkomt problemen met lange paden tijdens het compileren.
$env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA "gameboost-target"
Write-Host "CARGO_TARGET_DIR = $env:CARGO_TARGET_DIR"
