# Uploadt bestanden naar de openbare bucket "updates" in Supabase Storage (overschrijft bestaande).
# Wordt gebruikt door publish.ps1 en door de GitHub Actions-workflow release.yml.
# Gebruik:  .\scripts\upload-supabase.ps1 -ProjectRef abcd1234 -ServiceRoleKey eyJ... -Files a.exe, a.exe.sig, latest.json
param(
    [Parameter(Mandatory = $true)][string]$ProjectRef,
    [Parameter(Mandatory = $true)][string]$ServiceRoleKey,
    [Parameter(Mandatory = $true)][string[]]$Files,
    [string]$Bucket = "updates"
)
$ErrorActionPreference = "Stop"

foreach ($file in $Files) {
    if (-not (Test-Path $file)) { throw "Bestand niet gevonden: $file" }
    $name = Split-Path $file -Leaf
    $type = if ($name.EndsWith(".json")) { "application/json" } elseif ($name.EndsWith(".sig")) { "text/plain" } else { "application/octet-stream" }
    # latest.json mag maar kort in de cache blijven, anders ziet de app een nieuwe versie pas laat.
    $cache = if ($name -eq "latest.json") { "60" } else { "3600" }
    $headers = @{
        "apikey"        = $ServiceRoleKey
        "x-upsert"      = "true"
        "cache-control" = $cache
    }
    # Oude service_role-sleutel (JWT, begint met eyJ) ook als Bearer meesturen. Een nieuwe secret key
    # (sb_secret_...) is geen JWT: die alleen als apikey, Supabase zet hem zelf om.
    if (-not $ServiceRoleKey.StartsWith("sb_")) { $headers["Authorization"] = "Bearer $ServiceRoleKey" }
    $url = "https://$ProjectRef.supabase.co/storage/v1/object/$Bucket/$name"
    Write-Host "Uploaden: $name ($([math]::Round((Get-Item $file).Length / 1MB, 1)) MB)"
    Invoke-RestMethod -Method Post -Uri $url -Headers $headers -ContentType $type -InFile $file -TimeoutSec 600 | Out-Null
}
Write-Host "Klaar: $($Files.Count) bestand(en) in bucket '$Bucket'."
