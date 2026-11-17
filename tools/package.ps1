# Собирает релиз и кладёт готовый архив в dist/.
#
#     tools/package.ps1
#
# Внутри архива — два exe, иконка ярлыка, скрипт автозапуска и инструкция.

[CmdletBinding()]
param(
    [string]$Out = 'dist'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    $version = (Select-String -Path 'Cargo.toml' -Pattern '^version = "([^"]+)"' |
        Select-Object -First 1).Matches[0].Groups[1].Value
    if (-not $version) { throw 'Не удалось прочитать версию из Cargo.toml' }

    Write-Host '== иконки ==' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'icon.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'сборка иконок не удалась' }

    Write-Host '== cargo build --release ==' -ForegroundColor Cyan
    cargo build --release --bins
    if ($LASTEXITCODE -ne 0) { throw 'cargo build: сборка не удалась' }

    $name = "pithos-pandoras-$version-windows-x64"
    $dir = Join-Path $Out $name
    if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }

    New-Item -ItemType Directory -Path (Join-Path $dir 'assets') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $dir 'tools') -Force | Out-Null

    Copy-Item 'target\release\pithos-pandoras.exe' $dir
    Copy-Item 'target\release\pithos-pandoras-tray.exe' $dir
    Copy-Item 'docs\УСТАНОВКА.md' (Join-Path $dir 'ПРОЧТИ-МЕНЯ.md')
    Copy-Item 'assets\icon.ico' (Join-Path $dir 'assets')
    Copy-Item 'tools\install-tray.ps1' (Join-Path $dir 'tools')

    $zip = Join-Path $Out "$name.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path $dir -DestinationPath $zip

    $size = [math]::Round((Get-Item $zip).Length / 1MB, 1)
    Write-Host "Готово: $zip ($size МБ)" -ForegroundColor Green
} finally {
    Pop-Location
}
