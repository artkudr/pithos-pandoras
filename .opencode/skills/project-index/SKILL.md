---
name: project-index
description: >-
  Индекс проекта PithosPandoras: MCP graphify для структуры, вызовов и влияния,
  rg для точных строк. Загружай перед поиском по коду и после крупных правок.
---

# Индекс проекта

Сначала граф, потом чтение файлов. `rg` — для точных строк, а не для вопросов
«кто это вызывает».

## Когда что

| Вопрос | Инструмент |
|---|---|
| Из чего состоит модуль | `query_graph "..."` |
| Кто вызывает функцию | `get_neighbors "symbol"` |
| Что сломается от правки | `get_neighbors "symbol"` |
| Как связаны две сущности | `shortest_path "A" "B"` |
| Что это за узел | `get_node "symbol"` |
| Главные узлы | `god_nodes` |
| Размер и свежесть графа | `graph_stats` |
| Точный текст ошибки, ключ конфига | `rg -n "..." src` |
| Зависимости крейтов | `cargo tree` / `cargo metadata` |

## Граф

MCP-сервер `graphify` подключён проектом — `.opencode/opencode.json`, граф —
`graphify-out/graph.json`. Инструменты: `query_graph`, `get_node`,
`get_neighbors`, `get_community`, `god_nodes`, `graph_stats`, `shortest_path`.

В терминале без OpenCode — тот же граф через CLI: `graphify query`,
`graphify explain`, `graphify affected`, `graphify path`.

## Запасной путь

Если MCP недоступен или граф устарел:

```text
rg -n "pattern" src
cargo metadata --no-deps --format-version 1
cargo tree
```

Не выдавай догадку за результат графа. Если графа нет, скажи об этом и используй
`rg`.

## Обновление

- После переименования функции или крупной правки — `graphify update .`.
- Полная сборка — `graphify extract . --code-only --cargo`, затем
  `graphify cluster-only . --no-label`.
- После коммита граф обновляет git-хук; `graph_stats` показывает, на каком
  коммите он собран.
- `graphify-out/` в git не коммитится: это локальный кэш.

## Границы

- graphify разбирает AST локально, без API-ключа (`--code-only`).
- Макросы, трейты и динамическая диспетчеризация могут разрешаться неточно.
  Спорный вывод проверяй по исходнику.
- Индекс не заменяет `cargo check` и тесты.

Подробности — `docs/ИНДЕКС.md`.
