[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('build', 'update', 'query', 'explain', 'path', 'affected', 'status', 'check')]
    [string]$Command = 'check',

    [Parameter(Position = 1)]
    [string]$Query,

    [Parameter(Position = 2)]
    [string]$To,

    [int]$Depth = 3,
    [int]$Budget = 4000,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    $graphify = Get-Command graphify -ErrorAction SilentlyContinue

    if (-not $graphify) {
        Write-Host 'graphify не найден.' -ForegroundColor Yellow
        Write-Host 'Установите: uv tool install graphifyy' -ForegroundColor Yellow
        Write-Host 'Пока доступен запасной путь: rg, cargo metadata, cargo tree.' -ForegroundColor Yellow
        if ($Command -notin @('status', 'check')) { exit 2 }
    }

    switch ($Command) {
        'build' {
            Write-Host 'Строю граф (code-only, без API-ключа)...' -ForegroundColor Cyan
            & graphify extract . --code-only --cargo
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

            Write-Host 'Собираю отчёт и карту...' -ForegroundColor Cyan
            & graphify cluster-only . --no-label
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            & graphify export html
            exit $LASTEXITCODE
        }
        'update' {
            Write-Host 'Обновляю граф...' -ForegroundColor Cyan
            if ($Force) {
                & graphify update . --force
            } else {
                & graphify update .
            }
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

            Write-Host 'Обновляю отчёт и карту...' -ForegroundColor Cyan
            & graphify cluster-only . --no-label
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            & graphify export html
            exit $LASTEXITCODE
        }
        'query' {
            if (-not $Query) { throw 'Укажите вопрос: tools/index.ps1 query "..."' }
            & graphify query $Query --budget $Budget
            exit $LASTEXITCODE
        }
        'explain' {
            if (-not $Query) { throw 'Укажите имя: tools/index.ps1 explain "Symbol"' }
            & graphify explain $Query
            exit $LASTEXITCODE
        }
        'path' {
            if (-not $Query -or -not $To) { throw 'Укажите два имени: tools/index.ps1 path "A" "B"' }
            & graphify path $Query $To
            exit $LASTEXITCODE
        }
        'affected' {
            if (-not $Query) { throw 'Укажите имя: tools/index.ps1 affected "Symbol" -Depth 3' }
            & graphify affected $Query --depth $Depth
            exit $LASTEXITCODE
        }
        'status' {
            $graph = Join-Path $root 'graphify-out\graph.json'
            if (Test-Path $graph) {
                $info = Get-Item $graph
                Write-Host "graph.json: $($info.FullName)"
                Write-Host "Размер: $([math]::Round($info.Length / 1KB, 1)) КБ"
                Write-Host "Обновлён: $($info.LastWriteTime)"
            } else {
                Write-Host 'Граф не построен. Запустите: tools/index.ps1 build' -ForegroundColor Yellow
                exit 1
            }
        }
        'check' {
            if ($graphify) { & graphify --version }
            $graph = Join-Path $root 'graphify-out\graph.json'
            if (Test-Path $graph) {
                Write-Host 'Граф на месте.' -ForegroundColor Green
            } else {
                Write-Host 'Граф не построен.' -ForegroundColor Yellow
                exit 1
            }
        }
    }
} finally {
    Pop-Location
}
