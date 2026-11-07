# Compile the real NSIS recovery gate and run it against three disposable helpers.
param([string]$MakeNsis = (Join-Path $env:LOCALAPPDATA "tauri\NSIS\makensis.exe"))
$ErrorActionPreference = "Stop"
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$dir = Join-Path $tempRoot ("nezty-uninstall-test-" + [guid]::NewGuid())
$null = New-Item -Type Directory -Path $dir

function Compile([string]$name, [string]$source) {
    $path = Join-Path $dir "$name.nsi"
    [IO.File]::WriteAllText($path, $source)
    & $MakeNsis /V2 $path
    if ($LASTEXITCODE -ne 0) { throw "NSIS compilation failed: $name" }
}

try {
    foreach ($status in 0, 1) {
        Compile "helper-$status" @"
Unicode true
Name "Mock restore"
OutFile "helper-$status.exe"
RequestExecutionLevel user
SilentInstall silent
Section
SetErrorLevel $status
SectionEnd
"@
    }
    $hook = (Resolve-Path (Join-Path $PSScriptRoot "../src-tauri/installer-hooks.nsh")).Path
    $source = @'
Unicode true
!include LogicLib.nsh
!include "HOOKPATH"
Name "Uninstall recovery test"
OutFile "gate.exe"
RequestExecutionLevel user
SilentInstall silent
Var UpdateMode
Section
StrCpy $INSTDIR $EXEDIR
StrCpy $UpdateMode 0
!insertmacro NSIS_HOOK_PREUNINSTALL
FileOpen $0 "$EXEDIR\continued" w
FileClose $0
SectionEnd
'@
    Compile "gate" ($source.Replace("HOOKPATH", $hook))
    foreach ($case in "missing", "1", "0") {
        if ($case -ne "missing") {
            Copy-Item -LiteralPath (Join-Path $dir "helper-$case.exe") -Destination (Join-Path $dir "nezty.exe") -Force
        }
        $run = Start-Process -FilePath (Join-Path $dir "gate.exe") -ArgumentList "/S" -WindowStyle Hidden -Wait -PassThru
        $continued = Test-Path -LiteralPath (Join-Path $dir "continued")
        if (($case -eq "0") -ne $continued -or ($case -eq "0") -ne ($run.ExitCode -eq 0)) {
            throw "Unexpected result for restore=$case, exit=$($run.ExitCode), continued=$continued"
        }
        Write-Host "restore=$case exit=$($run.ExitCode) continued=$continued"
    }
} finally {
    $resolved = (Resolve-Path -LiteralPath $dir).Path
    if ([IO.Path]::GetDirectoryName($resolved) -eq $tempRoot.TrimEnd('\') -and
        [IO.Path]::GetFileName($resolved) -match '^nezty-uninstall-test-[\da-f-]{36}$') {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
