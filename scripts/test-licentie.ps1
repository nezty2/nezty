# Test van de licentiecontrole op een echte Windows-pc, stap voor stap, met de echte server.
# Laat bij elke stap zien wat er in het herstellogboek staat en of de registerwaarden echt terug zijn.
#
# Nodig: een geïnstalleerde nezty (met licentie gebouwd), NEZTY_LICENSE_URL en NEZTY_ADMIN_KEY voor de generator,
# en een testaccount. Het script maakt zelf een testsleutel. Gebruik:
#   .\scripts\test-licentie.ps1                 (alle stappen, met vragen tussendoor)
#   .\scripts\test-licentie.ps1 -Stap intrekken (één stap)
# Stappen: uitgelogd, intrekken, verwijderen, verlopen, offline, verwijderen-app, update
# Voor de zelftests (--selftest-license) is een testbuild nodig: .\scripts\build.ps1 -Selftest
param([string]$Stap = "alles")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$dataDir = Join-Path $env:ProgramFiles "Nezty Recovery\$sid"
$journal = Join-Path $dataDir "journal.json"
$exe = Join-Path $env:ProgramFiles "nezty\nezty.exe"
if (-not (Test-Path -LiteralPath $exe)) { throw "Installeer eerst de klantbuild in Program Files." }

function Kop($t) { Write-Host ""; Write-Host "=== $t ===" -ForegroundColor Cyan }
function Wacht($t) { Write-Host ""; Read-Host "  $t  (Enter om door te gaan)" | Out-Null }

# Registerwaarde lezen zoals hij nu is (of "(afwezig)").
function RegNu($hive, $path, $name) {
    $roots = @{ hklm = [Microsoft.Win32.Registry]::LocalMachine; hkcu = [Microsoft.Win32.Registry]::CurrentUser; hku = [Microsoft.Win32.Registry]::Users }
    $sub = $roots[$hive].OpenSubKey($path)
    try {
        if (!$sub -or $sub.GetValueNames() -notcontains $name) { return '{"t":"Absent"}' }
        $value = $sub.GetValue($name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $kind = $sub.GetValueKind($name).ToString()
        $types = @{ DWord='Dword'; QWord='Qword'; String='Str'; ExpandString='ExpandStr'; MultiString='MultiStr'; Binary='Binary' }
        if ($kind -eq 'DWord') { $value = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]$value), 0) }
        if ($kind -eq 'QWord') { $value = [BitConverter]::ToUInt64([BitConverter]::GetBytes([int64]$value), 0) }
        if ($kind -eq 'Binary') { $value = @($value | ForEach-Object { [int]$_ }) }
        return (ConvertTo-Json -Compress ([ordered]@{t=$types[$kind];v=$value}))
    } finally { if ($sub) { $sub.Dispose() } }
}

# Alle actieve (niet teruggezette) registerwijzigingen uit het logboek, met oude en huidige waarde.
function Logboek($titel) {
    Kop $titel
    if (-not (Test-Path $journal)) { Write-Host "  Geen logboek ($journal)."; return @() }
    $j = Get-Content $journal -Raw | ConvertFrom-Json
    $entries = if ($j.entries) { $j.entries } else { $j }
    $active = @($entries | Where-Object { $_.revertable -and -not $_.reverted })
    Write-Host "  Actieve wijzigingen in het logboek: $($active.Count)"
    foreach ($e in $active) {
        $line = "   - $($e.tweakName): $($e.target)"
        if ($e.payload -and $e.payload.kind -eq "registry") {
            $nu = RegNu $e.payload.hive $e.payload.path $e.payload.name
            $line += "  | oud: $($e.oldValue)  nu: $nu"
        }
        Write-Host $line
    }
    return $active
}

# Na terugzetten: elke registerwaarde moet weer de oude waarde hebben.
function Controleer($before) {
    $j = Get-Content $journal -Raw | ConvertFrom-Json
    $entries = if ($j.entries) { $j.entries } else { $j }
    $left = @($entries | Where-Object { $_.revertable -and -not $_.reverted }).Count
    $ok = $left -eq 0
    $seen = @{}
    foreach ($e in $before) {
        if ($e.payload -and $e.payload.kind -eq "registry") {
            $nu = RegNu $e.payload.hive $e.payload.path $e.payload.name
            $target = @($e.payload.hive, $e.payload.path, $e.payload.name) -join '|'
            if ($seen.ContainsKey($target)) { continue }
            $seen[$target] = $true
            if ($e.payload.old.t -eq 'Absent') { $expected = '{"t":"Absent"}' }
            else { $expected = ConvertTo-Json -Compress ([ordered]@{t=$e.payload.old.t;v=$e.payload.old.v}) }
            $terug = $nu -ceq $expected
            Write-Host ("   {0} {1}: nu {2}" -f ($(if ($terug) { "OK " } else { "MIS" })), $e.target, $nu)
            if (-not $terug) { $ok = $false }
        }
    }
    Write-Host ("  Nog actief in logboek: {0}  ->  {1}" -f $left, $(if ($ok) { "ALLES TERUG" } else { "NIET ALLES TERUG, kijk hierboven" })) -ForegroundColor $(if ($ok) { "Green" } else { "Red" })
}

function Sleutel {
    $out = npm run --silent licentie:maak -- --type maand --notitie "test-licentie.ps1" 2>&1 | Out-String
    $m = [regex]::Match($out, "nezty-[2-9a-hj-np-z]{4}-[2-9a-hj-np-z]{4}-[2-9a-hj-np-z]{3}")
    if (-not $m.Success) { throw "Kon geen testsleutel maken:`n$out" }
    return $m.Value
}

Kop "Voorbereiding"
Write-Host "  nezty: $exe"
Write-Host "  logboek: $journal"
$key = Sleutel
Write-Host "  Testsleutel: $key"
Wacht "Open nezty, maak een account met deze sleutel (vink 'Ingelogd blijven' aan) en zet twee of drie tweaks aan."
$before = Logboek "Stand vóór de test"
if ($before.Count -eq 0) { Write-Warning "Er staat niets aan. Zet eerst een tweak aan, anders valt er niets terug te zetten." }

if ($Stap -in "alles", "uitgelogd") {
    Kop "Test 0: uitloggen en herstarten zet niets terug"
    Write-Host "  Alleen een uitspraak van de server (of meer dan 24 uur zonder bevestiging) zet tweaks terug."
    Wacht "Log in nezty uit (of log in zonder 'Ingelogd blijven'), sluit nezty en herstart de pc"
    $nu = Logboek "Stand na uitloggen en herstart"
    $ok = $nu.Count -eq $before.Count
    Write-Host ("  Tweaks gebleven: {0}" -f $(if ($ok) { "JA" } else { "NEE, er is iets teruggezet" })) -ForegroundColor $(if ($ok) { "Green" } else { "Red" })
    $log = Join-Path $env:TEMP "nezty-license-check.json"
    if (Test-Path $log) { Write-Host "  Uitkomst van de controle bij het inloggen:"; Get-Content $log }
    $taken = @("nezty autostart", "nezty licentiecontrole $sid") | Where-Object { schtasks /Query /TN $_ 2>$null }
    Write-Host ("  Geplande taken: {0} (licentiecontrole verplicht, autostart optioneel)" -f ($taken -join ", "))
    Wacht "Log in nezty weer in (met 'Ingelogd blijven')"
}

if ($Stap -in "alles", "intrekken") {
    Kop "Test 1: sleutel intrekken"
    npm run --silent licentie:intrekken -- --sleutel $key
    Write-Host "  nezty controleert elke 15 minuten. Klik in nezty op Licentie > 'Opnieuw controleren' om niet te wachten."
    Wacht "Wacht tot nezty 'Licentie ingetrokken' toont"
    Controleer $before
    npm run --silent licentie:verleng -- --sleutel $key --dagen 30 | Out-Null
    Write-Host "  (Sleutel weer geldig gemaakt voor de volgende test.)"
    Wacht "Log in nezty opnieuw in en zet dezelfde tweaks weer aan"
    $before = Logboek "Stand"
}

if ($Stap -in "alles", "verwijderen") {
    Kop "Test 2: sleutel verwijderen"
    npm run --silent licentie:verwijder -- --sleutel $key
    Wacht "Klik in nezty op 'Opnieuw controleren' en wacht tot 'Licentie ingetrokken' verschijnt"
    Controleer $before
    $key = Sleutel
    Write-Host "  Nieuwe testsleutel: $key"
    Wacht "Maak in nezty opnieuw een account met deze sleutel en zet dezelfde tweaks weer aan"
    $before = Logboek "Stand"
}

if ($Stap -in "alles", "verlopen") {
    Kop "Test 3: licentie laten verlopen"
    Write-Host "  Een maandlicentie verloopt pas na 30 dagen. Voor de test: maak een demo-account (2 uur) of gebruik"
    Write-Host "  de testserver (npm run licentie:testserver) met een korte looptijd. Zet daar tweaks aan en wacht tot het einde."
    Wacht "Wacht tot nezty 'Licentie verlopen' toont"
    Controleer $before
    Wacht "Log weer in met een geldige licentie en zet dezelfde tweaks aan"
    $before = Logboek "Stand"
}

if ($Stap -in "alles", "offline") {
    Kop "Test 4: 24 uur offline"
    Write-Host "  Zet de pc-klok 25 uur vooruit terwijl nezty draait (of blokkeer nezty.exe 24 uur in de firewall)."
    Write-Host "  Het token geldt tot 'controleren vóór' (24 uur na de laatste controle). Daarna: 'Maak verbinding met internet' en alles terug."
    Wacht "Wacht tot nezty 'Maak verbinding met internet' toont"
    Controleer $before
    Wacht "Zet de klok terug, klik 'Opnieuw controleren' en zet dezelfde tweaks weer aan"
    $before = Logboek "Stand"
}

if ($Stap -in "alles", "verwijderen-app") {
    Kop "Test 5: app verwijderen"
    Write-Host "  Verwijder nezty via Windows Instellingen > Apps. De uninstaller vraagt één keer om toestemming (UAC)."
    Wacht "Verwijder nezty nu"
    $log = Join-Path $env:TEMP "nezty-restore-all.json"
    if (Test-Path $log) { Write-Host "  Uitkomst van --restore-all:"; Get-Content $log }
    else { Write-Warning "  $log ontbreekt: --restore-all is niet gedraaid (UAC geweigerd?)" }
    $seen = @{}
    foreach ($e in $before) {
        if ($e.payload -and $e.payload.kind -eq "registry") {
            Write-Host ("   {0}: nu {1} (oud: {2})" -f $e.target, (RegNu $e.payload.hive $e.payload.path $e.payload.name), $e.oldValue)
        }
    }
    Write-Host ("  Gegevensmap weg: {0}" -f (-not (Test-Path $dataDir)))
    Write-Host ("  Taak 'nezty licentiecontrole' weg: {0}" -f ((schtasks /Query /TN "nezty licentiecontrole" 2>$null) -eq $null))
    Write-Host ("  Taak 'nezty autostart' weg: {0}" -f ((schtasks /Query /TN "nezty autostart" 2>$null) -eq $null))
    Write-Host ("  Token weg: {0}" -f ((cmdkey /list 2>$null | Select-String "nezty/license") -eq $null))
}

if ($Stap -in "alles", "update") {
    Kop "Test 6: update uitbrengen"
    Write-Host "  Verhoog de versie, commit en push naar main. Kijk onder Actions (release.yml) of de run groen is."
    Write-Host "  Open daarna nezty: binnen 30 minuten (of meteen na herstart) verschijnt 'Update vereist', daarna 'nezty is bijgewerkt naar versie X'."
    Write-Host "  Controleer ook met een verlopen of ingetrokken licentie: de update komt dan ook binnen."
}

Kop "Klaar"
Write-Host "  Ruim de testsleutel op met:  npm run licentie:verwijder -- --sleutel $key"
