$ErrorActionPreference = 'Stop'

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $shell = if ($PSVersionTable.PSEdition -eq 'Core') {
        Join-Path $PSHOME 'pwsh.exe'
    } else {
        Join-Path $PSHOME 'powershell.exe'
    }
    Start-Process -FilePath $shell -Verb RunAs -ArgumentList @(
        '-NoProfile', '-NoExit', '-File', ('"{0}"' -f $PSCommandPath)
    )
    return
}

$retailClients = @(Get-Process -Name Gw -ErrorAction SilentlyContinue)
if ($retailClients.Count -eq 0) {
    throw 'Open retail Gw.exe and leave it on the login screen, then run this script again.'
}
if ($retailClients.Count -gt 1) {
    throw 'More than one retail Gw.exe is open. Leave only the client you want to record running.'
}

$python = Join-Path $env:LOCALAPPDATA 'Programs\Python\Python313-32\python.exe'
$py4gwRoot = if ($env:PY4GW_STEALTH) {
    $env:PY4GW_STEALTH
} else {
    Join-Path $env:USERPROFILE 'Py4GW_Stealth'
}
if (-not (Test-Path -LiteralPath $python -PathType Leaf)) {
    throw "32-bit Python was not found at $python"
}
if (-not (Test-Path -LiteralPath $py4gwRoot -PathType Container)) {
    throw "Py4GW Stealth was not found at $py4gwRoot"
}

$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$rawDirectory = Join-Path $repo 'recordings\client\raw'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
$output = Join-Path $rawDirectory "fresh-character-$stamp.jsonl"
$sniffer = Join-Path $PSScriptRoot 'sniff-capture.py'

Write-Host 'Wait for "--- capturing", then log in and create the character.'
Write-Host 'After the first map loads, open Titles and press Ctrl+C here.'
& $python -u $sniffer --pid $retailClients[0].Id --dir both --seconds 0 --raw --py4gw-root $py4gwRoot --out $output
