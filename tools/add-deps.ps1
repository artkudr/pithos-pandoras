[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'cargo не найден. Установите Rust: https://rustup.rs'
    }

    cargo add tokio --features full
    cargo add axum
    cargo add rmcp --features transport-streamable-http-server
    cargo add rusqlite --features bundled
    cargo add askama
    cargo add serde --features derive
    cargo add serde_json
    cargo add tracing
    cargo add tracing-subscriber --features env-filter
    cargo add uuid --features v7,serde
    cargo add time --features serde,formatting,parsing,macros
    cargo add clap --features derive
    cargo add rust-embed
    cargo add thiserror
    cargo add anyhow
    cargo add --dev tempfile

    Write-Host ''
    Write-Host 'Базовые зависимости добавлены.' -ForegroundColor Green
    Write-Host 'Для rmcp включён HTTP-транспорт transport-streamable-http-server.' -ForegroundColor Green
} finally {
    Pop-Location
}
