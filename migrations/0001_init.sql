-- PithosPandoras, схема M1.
-- Только вперёд: новые изменения — отдельным файлом.

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    root_path TEXT NOT NULL,
    tasks_dir TEXT NOT NULL,
    folder_pattern TEXT NOT NULL,
    config_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS workspace_counters (
    workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
    next_number INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workspace_statuses (
    workspace_id TEXT NOT NULL REFERENCES workspaces(id),
    id TEXT NOT NULL,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,
    color TEXT,
    sort INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (workspace_id, id)
);

CREATE TABLE IF NOT EXISTS workspace_roles (
    workspace_id TEXT NOT NULL REFERENCES workspaces(id),
    name TEXT NOT NULL,
    color TEXT,
    sort INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (workspace_id, name)
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id),
    number INTEGER NOT NULL,
    folder_name TEXT NOT NULL,
    title TEXT NOT NULL,
    statement TEXT NOT NULL,
    status_id TEXT NOT NULL,
    route TEXT,
    route_snapshot TEXT,
    resource TEXT,
    waiting_for TEXT NOT NULL DEFAULT '',
    waiting_ref TEXT NOT NULL DEFAULT '',
    waiting_since TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT,
    origin TEXT NOT NULL,
    UNIQUE (workspace_id, number),
    FOREIGN KEY (workspace_id, status_id)
        REFERENCES workspace_statuses(workspace_id, id)
);

CREATE TABLE IF NOT EXISTS stages (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    role TEXT NOT NULL,
    round INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    end_reason TEXT,
    verdict TEXT NOT NULL DEFAULT '',
    seconds INTEGER,
    note TEXT NOT NULL DEFAULT '',
    UNIQUE (task_id, role, round)
);

CREATE TABLE IF NOT EXISTS reports (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    stage_id TEXT REFERENCES stages(id),
    actor TEXT,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK (stage_id IS NOT NULL OR actor IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    stage_id TEXT REFERENCES stages(id),
    report_id TEXT REFERENCES reports(id),
    actor TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    path TEXT NOT NULL,
    mime TEXT,
    size INTEGER,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    seq INTEGER NOT NULL,
    at TEXT NOT NULL,
    kind TEXT NOT NULL,
    stage_id TEXT REFERENCES stages(id),
    actor TEXT,
    payload TEXT NOT NULL DEFAULT '{}',
    origin TEXT NOT NULL,
    CHECK (stage_id IS NOT NULL OR actor IS NOT NULL),
    UNIQUE (task_id, seq)
);

CREATE TABLE IF NOT EXISTS comments (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    author TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS stages_open
    ON stages(task_id) WHERE finished_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS reports_stage_kind
    ON reports(stage_id, kind) WHERE stage_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS tasks_workspace_status
    ON tasks(workspace_id, status_id);

CREATE INDEX IF NOT EXISTS stages_task_role
    ON stages(task_id, role, round);

CREATE INDEX IF NOT EXISTS reports_task
    ON reports(task_id);

CREATE INDEX IF NOT EXISTS files_task
    ON files(task_id);

CREATE INDEX IF NOT EXISTS events_task_seq
    ON events(task_id, seq);

CREATE INDEX IF NOT EXISTS comments_task
    ON comments(task_id);
