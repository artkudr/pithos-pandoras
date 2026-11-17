[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    $manifest = 'tools/iconbuild/Cargo.toml'

    Write-Host '== PNG, RGBA для трея ==' -ForegroundColor Cyan
    cargo run --release --quiet --manifest-path $manifest -- `
        assets/icon.svg assets 16 24 32 48 64 128 180 256 512 --rgba 16 24 32
    if ($LASTEXITCODE -ne 0) { throw 'iconbuild: не удалось собрать PNG' }

    Write-Host '== icon.ico ==' -ForegroundColor Cyan
    cargo run --release --quiet --manifest-path $manifest -- `
        assets/icon.svg assets 16 24 32 48 64 128 256 --ico icon.ico
    if ($LASTEXITCODE -ne 0) { throw 'iconbuild: не удалось собрать icon.ico' }

    Copy-Item 'assets/icon.svg' 'src/web/static/icon.svg' -Force
    Copy-Item 'assets/icon-32.png' 'src/web/static/icon-32.png' -Force
    Copy-Item 'assets/icon-180.png' 'src/web/static/icon-180.png' -Force

    Write-Host 'Иконки готовы: assets/ и src/web/static/' -ForegroundColor Green
} finally {
    Pop-Location
}
