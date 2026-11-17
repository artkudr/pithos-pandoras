[CmdletBinding()]
param(
    [string]$Exe = ""
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Exe) {
    $candidates = @(
        (Join-Path $root 'pithos-pandoras-tray.exe'),
        (Join-Path $root 'target\release\pithos-pandoras-tray.exe')
    )
    $Exe = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $Exe -or -not (Test-Path $Exe)) {
    throw "Не найден pithos-pandoras-tray.exe. Соберите его или укажите путь через -Exe."
}

$startup = [Environment]::GetFolderPath('Startup')
$link = Join-Path $startup 'PithosPandoras.lnk'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($link)
$shortcut.TargetPath = $Exe
$shortcut.WorkingDirectory = $root
$icon = Join-Path $root 'assets\icon.ico'
if (Test-Path $icon) {
    $shortcut.IconLocation = $icon
}
$shortcut.Description = 'PithosPandoras — локальный сервис задач'
$shortcut.Save()

Write-Host "Ярлык создан: $link"
