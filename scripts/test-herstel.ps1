# Compile and run the Rust tests without elevating or starting the desktop app.
param([string]$Mt, [switch]$LicenseFixture)
$ErrorActionPreference = "Stop"
$manifest = Join-Path $PSScriptRoot "../src-tauri/Cargo.toml"
if (!$env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $env:TEMP "nezty-rust-tests" }
$build = & cargo test --manifest-path $manifest --lib --locked --no-run --message-format=json-render-diagnostics
if ($LASTEXITCODE -ne 0) { throw "Compilatie van hersteltests mislukt" }
$test = $build | ForEach-Object { $_ | ConvertFrom-Json } |
    Where-Object { $_.reason -eq "compiler-artifact" -and $_.profile.test -and $_.target.name -eq "nezty_lib" } |
    Select-Object -Last 1
if (!$test.executable) { throw "Testprogramma ontbreekt" }
if (!$Mt) {
    $Mt = (Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\mt.exe" |
        Sort-Object FullName | Select-Object -Last 1).FullName
}

# MinGW embeds its own default manifest in a Cargo unit-test harness.
# Tauri imports TaskDialogIndirect from Common Controls v6 at process startup.
$root = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$dir = Join-Path $root ("nezty-test-run-" + [guid]::NewGuid())
$null = New-Item -Type Directory -Path $dir
try {
    $exe = Join-Path $dir "tests.exe"
    Copy-Item -LiteralPath $test.executable -Destination $exe
    $debug = Split-Path (Split-Path $test.executable)
    Copy-Item -LiteralPath (Join-Path $debug "WebView2Loader.dll") -Destination $dir
    $appManifest = Get-Content (Join-Path $PSScriptRoot "../src-tauri/app.manifest") -Raw
    $testManifest = Join-Path $dir "tests.manifest"
    [IO.File]::WriteAllText($testManifest, $appManifest.Replace("requireAdministrator", "asInvoker"))
    & $Mt -nologo -manifest $testManifest "-outputresource:$exe;#1"
    if ($LASTEXITCODE -ne 0) { throw "Testmanifest toevoegen mislukt" }
    if ($LicenseFixture) { & $exe --include-ignored } else { & $exe }
    if ($LASTEXITCODE -ne 0) { throw "Hersteltests mislukt" }
} finally {
    $resolved = (Resolve-Path -LiteralPath $dir).Path
    if ([IO.Path]::GetDirectoryName($resolved) -eq $root.TrimEnd('\') -and
        [IO.Path]::GetFileName($resolved) -match '^nezty-test-run-[\da-f-]{36}$') {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
