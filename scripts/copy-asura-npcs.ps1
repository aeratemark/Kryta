# Copies ASURA's per-map NPC placements (npcs.json) into the matching Kryta map folders.
$src = 'D:\ASURA-main\config\World\Vanilla\maps\Prophecies\Ascalon (pre-Searing)'
$dst = 'D:\Kryta-main\config\World\Vanilla\maps\Prophecies\Ascalon (pre-Searing)'
Get-ChildItem -LiteralPath $src -Recurse -Filter npcs.json | ForEach-Object {
    $to = $dst + $_.FullName.Substring($src.Length)
    Copy-Item -LiteralPath $_.FullName -Destination $to
    Write-Host "copied $to"
}
