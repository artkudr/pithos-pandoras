---
name: project-index
description: >-
  Индекс проекта PithosPandoras: graphify для структуры, вызовов и влияния,
  rg для точных строк. Загружай перед поиском по коду и после крупных правок.
---

# Индекс проекта

Сначала граф, потом чтение файлов. `rg` — для точных строк, а не для вопросов
«кто это вызывает».

## Когда что

| Вопрос | Инструмент |
|---|---|
| Из чего состоит модуль | `tools/index.ps1 query "..."` |
| Кто вызывает функцию | `tools/index.ps1 affected "symbol"` |
| Что сломается от правки | `tools/index.ps1 affected "symbol" -Depth 3` |
| Как связаны две сущности | `tools/index.ps1 path "A" "B"` |
| Что это за узел | `tools/index.ps1 explain "symbol"` |
| Точный текст ошибки, ключ конфига | `rg -n "..." src` |
| Зависимости крейтов | `cargo tree` / `cargo metadata` |

## Команды

```text
tools/index.ps1 build                 # первый граф
tools/index.ps1 update                # после структурных правок
tools/index.ps1 query "как устроен writer"
tools/index.ps1 explain "AppError"
tools/index.ps1 path "stage_start" "reports"
tools/index.ps1 affected "create_task" -Depth 3
tools/index.ps1 status                # есть ли граф
```

## Запасной путь

Если `graphify` не установлен или граф устарел:

```text
rg -n "pattern" src
cargo metadata --no-deps --format-version 1
cargo tree
```

Не выдавай догадку за результат графа. Если графа нет, скажи об этом и используй
`rg`.

## Обновление

- После добавления модуля, переименования функции или крупной правки —
  `tools/index.ps1 update`.
- `build` — с нуля; `update` — только изменённые файлы.
- `graphify-out/` в git не коммитится: это локальный кэш.

## Границы

- graphify разбирает AST локально, без API-ключа (`--code-only`).
- Макросы, трейты и динамическая диспетчеризация могут разрешаться неточно.
  Спорный вывод проверяй по исходнику.
- Индекс не заменяет `cargo check` и тесты.

Подробности — `docs/ИНДЕКС.md`.
