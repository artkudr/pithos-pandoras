# PithosPandoras

Локальный сервис задач: один процесс, SQLite, MCP-сервер для агентов и страница
в браузере. Пишется на Rust.

Сейчас это каркас: правила, скиллы, индекс и сборочная обвязка. Реализация — по
этапам M1–M5 из архитектуры.

## С чего начать

1. `docs/Сервис-задач-архитектура.md` — что строим.
2. `docs/АРХИТЕКТУРА-КОДА.md` — как разложить код.
3. `docs/РАЗРАБОТКА.md` — сборка, проверки, зависимости.
4. `docs/ИНДЕКС.md` — как искать по проекту.

## Быстрый старт

Команды выполняются в PowerShell 7 (`pwsh`).

```text
tools/setup-dev.ps1

cargo run -- init --workspace PithosPandoras --root . --db pithos-pandoras.db
cargo run -- serve --db pithos-pandoras.db
cargo run -- doctor --db pithos-pandoras.db
cargo run -- backup --db pithos-pandoras.db

tools/check.ps1          # форматирование, clippy, тесты, cargo deny
```

Сервер для агентов: `http://127.0.0.1:8787/mcp`. Страница: `http://127.0.0.1:8787/`.
При первом запуске `serve` создаёт `pithos-pandoras.key` и печатает ключ.
Клиент MCP передаёт `Authorization: Bearer <ключ>`.

Реализованы M1–M4: доска, карточка, действия человека, поиск, статистика,
пространства, роли, статусы, входы и импорт. Плюс Windows-трей и копия базы
(`backup`). Остались события `/events` и упаковка.

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
нашлись. Пути можно переопределить переменными `PITHOS_DB`, `PITHOS_WORKSPACE`,
`PITHOS_PORT`, `PITHOS_KEY_FILE`.

## Индекс

```text
tools/index.ps1 build
tools/index.ps1 query "как устроен writer"
tools/index.ps1 affected "create_task" -Depth 3
```

`home-dev-kit/` — справочный архив с рабочего компьютера. Не часть проекта,
в индекс не попадает.
