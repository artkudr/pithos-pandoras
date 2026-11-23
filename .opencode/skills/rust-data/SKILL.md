---
name: rust-data
description: >-
  SQLite в PithosPandoras: rusqlite, WAL, один писатель, транзакции, миграции,
  FTS5, UUID v7. Загружай при работе с базой, схемой, запросами и конкурентностью.
---

# Данные и SQLite

Один писатель. Чтение параллельно. Транзакции короткие.

## Подключение

```rust
use rusqlite::Connection;
use std::time::Duration;

fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}
```

`rusqlite` собирается с `bundled`: SQLite внутри бинарника, внешних DLL нет.

## Один писатель

Writer — отдельный поток или задача, у которой единственное соединение на запись.
Все мутации идут через очередь команд.

```rust
enum WriteCommand {
    CreateTask { /* ... */, reply: tokio::sync::oneshot::Sender<Result<Task, AppError>> },
    StartStage { /* ... */, reply: tokio::sync::oneshot::Sender<Result<Stage, AppError>> },
}
```

Правила:

- Writer владеет `Connection` и не отдаёт его наружу.
- Номер задачи выдаётся в той же транзакции, что и вставка.
- Никаких `await` внутри транзакции.
- Ошибка writer'а возвращается вызывающему, а не логируется молча.
- Чтение — отдельный пул соединений или `spawn_blocking` с коротким соединением.

## Транзакции

```rust
let tx = conn.transaction()?;
let number: i64 = tx.query_row(
    "UPDATE project_counters
     SET next_number = next_number + 1
     WHERE project_id = ?1
     RETURNING next_number - 1",
    [project_id],
    |row| row.get(0),
)?;
// ... вставка задачи ...
tx.commit()?;
```

- Всё, что должно быть атомарным, — в одной транзакции.
- Не смешивай чтение «до» и запись «после» без транзакции, если между ними
  может вклиниться другой writer.
- Частичные уникальные индексы — часть контракта:
  `stages(task_id) WHERE finished_at IS NULL`,
  `reports(stage_id, kind) WHERE stage_id IS NOT NULL`.

## Миграции

- `migrations/0001_init.sql`, `0002_...sql` — только вперёд.
- Таблица `schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT)`.
- Миграция идемпотентна при повторном запуске: проверяй версию.
- Не удаляй и не переименовывай колонки без отдельного решения и записи в `docs/`.
- `taskd doctor` проверяет версию схемы и целостность.

## Идентификаторы и время

- `uuid` v7 для `tasks.id`, `stages.id`, `reports.id`, `files.id`, `comments.id`.
- `events.id` — автоинкремент; `events.uuid` — v7; `events.seq` — по задаче.
- Время — `time::OffsetDateTime`, в базе RFC3339 UTC.
- Местная дата — только для имени папки (`{date}`), не для хранения.

## Поиск

FTS5 — по названию, постановке, комментариям и текстам отчётов. Индекс
обновляется в той же транзакции, что и запись. Не делай отдельный «поисковый»
процесс.

## Ошибки

`rusqlite::Error` не отдаётся наружу как есть. В `store` он преобразуется в
`StoreError`, в сервисе — в `AppError` с кодом `io`, `conflict` или `not_found`.
Нарушение уникального индекса — `conflict`, а не `io`.

## Тесты

- Временная папка `tempfile::TempDir`, файл базы внутри.
- Миграции применяются в тесте с нуля.
- Конкурентные тесты: два `create_task` — разные номера; два `stage_start`
  одной ролью — тот же этап с `resumed: true`.
- Не используй общую базу между тестами.
