[CmdletBinding()]
param(
    [switch]$InstallGraphify
)

$ErrorActionPreference = 'Stop'

function Test-Tool {
    param([string]$Name)
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if ($cmd) {
        Write-Host "[ok] $Name -> $($cmd.Source)" -ForegroundColor Green
        return $true
    }
    Write-Host "[--] $Name не найден" -ForegroundColor Yellow
    return $false
}

$missing = @()

if (-not (Test-Tool 'rustup')) { $missing += 'rustup: https://rustup.rs' }
if (-not (Test-Tool 'cargo')) { $missing += 'Rust + Visual Studio Build Tools с C++: https://rustup.rs' }
if (-not (Test-Tool 'uv')) { $missing += 'uv: winget install astral-sh.uv' }
if (-not (Test-Tool 'graphify')) { $missing += 'graphify: uv tool install graphifyy' }
if (-not (Test-Tool 'git')) { $missing += 'Git: winget install Git.Git' }
if (-not (Test-Tool 'cargo-deny')) { $missing += 'cargo-deny: cargo install cargo-deny --locked' }
if (-not (Test-Tool 'rg')) { $missing += 'ripgrep: winget install BurntSushi.ripgrep.MSVC' }
if (-not (Test-Tool 'rust-analyzer')) { $missing += 'rust-analyzer: rustup component add rust-analyzer' }

if ($InstallGraphify -and -not (Get-Command graphify -ErrorAction SilentlyContinue)) {
    if (-not (Get-Command uv -ErrorAction SilentlyContinue)) { throw 'Сначала установите uv.' }
    uv tool install graphifyy
}

if ($missing.Count -gt 0) {
    Write-Host ''
    Write-Host 'Чего не хватает:' -ForegroundColor Cyan
    $missing | ForEach-Object { Write-Host " - $_" }
    exit 1
}

Write-Host 'Окружение готово.' -ForegroundColor Green
