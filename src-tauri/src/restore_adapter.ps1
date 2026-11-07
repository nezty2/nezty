# Restore the Device Manager power checkbox for the exact adapter from the journal.
param([string]$Name, [bool]$Enable)
$adapters = @(Get-CimInstance -ClassName Win32_NetworkAdapter |
    Where-Object { $_.NetConnectionID -eq $Name -and $_.PNPDeviceID })
if ($adapters.Count -ne 1) { throw "Adapter missing or ambiguous: $Name" }
$pnp = $adapters[0].PNPDeviceID
function Get-PowerDevice {
    Get-CimInstance -Namespace root\wmi -ClassName MSPower_DeviceEnable |
        Where-Object { $_.InstanceName -eq $pnp -or
            $_.InstanceName.StartsWith($pnp + '_', [StringComparison]::OrdinalIgnoreCase) }
}
$devices = @(Get-PowerDevice)
if ($devices.Count -ne 1) { throw "Power setting missing or ambiguous: $Name" }
if ($devices[0].Enable -ne $Enable) {
    Set-CimInstance -InputObject $devices[0] -Property @{ Enable = $Enable } -ErrorAction Stop
}
$after = @(Get-PowerDevice)
if ($after.Count -ne 1 -or $after[0].InstanceName -ne $devices[0].InstanceName -or $after[0].Enable -ne $Enable) {
    throw "Adapter power setting was not restored: $Name"
}
