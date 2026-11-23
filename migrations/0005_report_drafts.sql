-- Черновики отчётов и признак активности этапа.
-- Только вперёд: старые миграции не правим.

CREATE TABLE IF NOT EXISTS report_drafts (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    stage_id TEXT NOT NULL REFERENCES stages(id),
    actor TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    published_report_id TEXT REFERENCES reports(id)
);

CREATE TABLE IF NOT EXISTS report_draft_blocks (
    draft_id TEXT NOT NULL REFERENCES report_drafts(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL,
    sort INTEGER NOT NULL,
    PRIMARY KEY (draft_id, key)
);

CREATE INDEX IF NOT EXISTS report_drafts_task
    ON report_drafts(task_id, stage_id);

CREATE INDEX IF NOT EXISTS report_drafts_stage
    ON report_drafts(stage_id);

CREATE INDEX IF NOT EXISTS report_draft_blocks_sort
    ON report_draft_blocks(draft_id, sort);

ALTER TABLE stages ADD COLUMN last_activity_at TEXT;

UPDATE stages
SET last_activity_at = COALESCE(finished_at, started_at)
WHERE last_activity_at IS NULL;
