# Ondertekent één bestand (exe of installer). Tauri roept dit script aan voor elk bestand dat het bundelt.
# Instellingen komen uit scripts\signing.json (zie signing.example.json). Dat bestand staat niet in git.
#
# Methodes:
#   certstore : certificaat in de Windows-certificaatopslag (Huidige gebruiker > Persoonlijk), via de thumbprint.
#               Werkt met een gekocht OV-certificaat op een USB-token of in de cloud (SimplySign, eSigner),
#               zolang de leverancier het certificaat in de Windows-opslag zichtbaar maakt.
#   azure     : Azure Artifact Signing (vroeger Trusted Signing) via artifact-signing-cli.
#               Vereist AZURE_CLIENT_ID, AZURE_CLIENT_SECRET en AZURE_TENANT_ID als omgevingsvariabelen.
param([Parameter(Mandatory = $true)][string]$File)
$ErrorActionPreference = "Stop"

$configPath = Join-Path $PSScriptRoot "signing.json"
if (-not (Test-Path $configPath)) {
    throw "scripts\signing.json ontbreekt. Kopieer signing.example.json naar signing.json en vul je gegevens in."
}
$cfg = Get-Content $configPath -Raw | ConvertFrom-Json
$timestamp = if ($cfg.timestampUrl) { $cfg.timestampUrl } else { "http://timestamp.digicert.com" }

Write-Host "Ondertekenen: $File (methode: $($cfg.method))"

switch ($cfg.method) {
    "certstore" {
        if (-not $cfg.thumbprint) { throw "Vul 'thumbprint' in signing.json in." }
        $thumb = ($cfg.thumbprint -replace "\s", "").ToUpper()
        $cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Thumbprint -eq $thumb } | Select-Object -First 1
        if (-not $cert) { throw "Geen code-signing-certificaat met thumbprint $thumb gevonden in Huidige gebruiker > Persoonlijk." }
        $result = Set-AuthenticodeSignature -FilePath $File -Certificate $cert -HashAlgorithm SHA256 -TimestampServer $timestamp
        if (-not $result.SignerCertificate) { throw "Ondertekenen mislukt: $($result.StatusMessage)" }
        Write-Host "  Ondertekend door $($cert.Subject). Status: $($result.Status)"
    }
    "azure" {
        foreach ($v in "AZURE_CLIENT_ID", "AZURE_CLIENT_SECRET", "AZURE_TENANT_ID") {
            if (-not [Environment]::GetEnvironmentVariable($v)) { throw "Omgevingsvariabele $v ontbreekt." }
        }
        $cliArgs = @("-e", $cfg.endpoint, "-a", $cfg.account, "-c", $cfg.profile, "-d", "nezty", $File)
        & artifact-signing-cli @cliArgs
        if ($LASTEXITCODE -ne 0) { throw "artifact-signing-cli gaf foutcode $LASTEXITCODE." }
    }
    default { throw "Onbekende methode '$($cfg.method)'. Gebruik 'certstore' of 'azure'." }
}
