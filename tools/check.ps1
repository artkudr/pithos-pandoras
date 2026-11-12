[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    Write-Host '== cargo fmt ==' -ForegroundColor Cyan
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'cargo fmt: есть неотформатированные файлы' }

    Write-Host '== cargo clippy ==' -ForegroundColor Cyan
    cargo clippy --all-targets --all-features -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'cargo clippy: есть предупреждения' }

    Write-Host '== cargo test ==' -ForegroundColor Cyan
    cargo test --all-features
    if ($LASTEXITCODE -ne 0) { throw 'cargo test: есть падения' }

    if (Get-Command cargo-deny -ErrorAction SilentlyContinue) {
        Write-Host '== cargo deny ==' -ForegroundColor Cyan
        cargo deny check --hide-inclusion-graph
        if ($LASTEXITCODE -ne 0) { throw 'cargo deny: есть нарушения' }
    } else {
        Write-Host 'cargo-deny не установлен, проверка лицензий пропущена.' -ForegroundColor Yellow
    }

    Write-Host 'Все проверки пройдены.' -ForegroundColor Green
} finally {
    Pop-Location
}
