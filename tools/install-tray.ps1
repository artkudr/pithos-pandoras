[CmdletBinding()]
param(
    [string]$Exe = ""
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Exe) {
    $Exe = Join-Path $root 'target\release\pithos-pandoras-tray.exe'
}
if (-not (Test-Path $Exe)) {
    throw "Не найден $Exe. Сначала: cargo build --release --bin pithos-pandoras-tray"
}

$startup = [Environment]::GetFolderPath('Startup')
$link = Join-Path $startup 'PithosPandoras.lnk'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($link)
$shortcut.TargetPath = $Exe
$shortcut.WorkingDirectory = $root
$shortcut.IconLocation = $Exe
$shortcut.Description = 'PithosPandoras — локальный сервис задач'
$shortcut.Save()

Write-Host "Ярлык создан: $link"
