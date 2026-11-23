# PithosPandoras

Локальный сервис задач: один процесс, SQLite, MCP-сервер для агентов и страница
в браузере. Пишется на Rust.

Реализованы M1–M4 и Windows-сборка в архиве. Остались Linux и macOS.

## Где взять

- Установка — из GitHub Releases этого репозитория: zip последней версии,
  дальше по `docs/УСТАНОВКА.md`.
- Проверка после запуска: `http://127.0.0.1:8787/health` отвечает `ok`.
- Ключ для агентов печатает `pithos-pandoras.exe key`.
- Внешнего реестра нет: единственный источник сборки — Releases этого
  репозитория. Новый релиз — новый тег и новый zip в нём.

## С чего начать

1. `docs/Сервис-задач-архитектура.md` — что строим.
2. `docs/АРХИТЕКТУРА-КОДА.md` — как разложить код.
3. `docs/РАЗРАБОТКА.md` — сборка, проверки, зависимости.
4. `docs/ИНДЕКС.md` — как искать по проекту.
5. `docs/УСТАНОВКА.md` — как поставить и запустить готовую сборку.

## Быстрый старт

Команды выполняются в PowerShell 7 (`pwsh`).

```text
tools/setup-dev.ps1

cargo run -- init --project PithosPandoras --root . --db pithos-pandoras.db
cargo run -- serve --db pithos-pandoras.db
cargo run -- doctor --db pithos-pandoras.db
cargo run -- key                    # ключ доступа MCP
cargo run -- backup --db pithos-pandoras.db

tools/check.ps1          # форматирование, clippy, тесты, cargo deny
```

Сервер для агентов: `http://127.0.0.1:8787/mcp`. Страница:
`http://127.0.0.1:8787/` — редирект в проект по умолчанию; список проектов —
`/projects`, доска проекта — `/p/{project}`.
При первом запуске `serve` создаёт `pithos-pandoras.key` и печатает ключ.
Показать ключ в любой момент — `cargo run -- key` (печатает только ключ).
Клиент MCP передаёт `Authorization: Bearer <ключ>`.

Есть также Windows-трей, копия базы (`backup`) и живое обновление страницы
(`/events`).

Зависимости уже добавлены. Если понадобится повторить набор на чистом каркасе —
`tools/add-deps.ps1`.

## Windows-сборка и трей

```text
cargo build --release --bin pithos-pandoras-tray
.\target\release\pithos-pandoras-tray.exe
```

Приложение запускает сервер, показывает иконку в трее и открывает страницу по
клику. Консольного окна нет; лог — `pithos-pandoras.log`.

Автозапуск при входе:

```text
tools/install-tray.ps1
```

Скрипт создаёт ярлык в Startup с рабочим каталогом проекта, чтобы база и ключ
нашлись. Пути можно переопределить переменными `PITHOS_DB`, `PITHOS_PROJECT`,
`PITHOS_PORT`, `PITHOS_KEY_FILE`.

Готовый архив для выдачи:

```text
tools/package.ps1
```

Скрипт собирает иконки и релиз, кладёт в `dist/` папку и zip
`pithos-pandoras-<версия>-windows-x64.zip`: два exe, иконка ярлыка, скрипт
автозапуска и `ПРОЧТИ-МЕНЯ.md`. Версия берётся из `Cargo.toml`.

## Индекс

Граф через MCP `graphify` (`.opencode/opencode.json`):

```text
query_graph "как устроен writer"
get_node "AppError"
get_neighbors "create_task"
graph_stats
```

В терминале без OpenCode — `graphify query "…"`, `graphify explain "…"`,
`graphify affected "…"`.

`home-dev-kit/` — справочный архив с рабочего компьютера. Не часть проекта,
в индекс не попадает.
