-- Полнотекстовый поиск M1.

CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(
    task_id UNINDEXED,
    title,
    statement,
    tokenize = 'unicode61'
);

CREATE VIRTUAL TABLE IF NOT EXISTS reports_fts USING fts5(
    report_id UNINDEXED,
    task_id UNINDEXED,
    title,
    content,
    tokenize = 'unicode61'
);

CREATE VIRTUAL TABLE IF NOT EXISTS comments_fts USING fts5(
    comment_id UNINDEXED,
    task_id UNINDEXED,
    body,
    tokenize = 'unicode61'
);

INSERT INTO tasks_fts(rowid, task_id, title, statement)
SELECT rowid, id, title, statement FROM tasks;

INSERT INTO reports_fts(rowid, report_id, task_id, title, content)
SELECT rowid, id, task_id, title, content FROM reports;

INSERT INTO comments_fts(rowid, comment_id, task_id, body)
SELECT rowid, id, task_id, body FROM comments;

CREATE TRIGGER IF NOT EXISTS tasks_ai AFTER INSERT ON tasks BEGIN
    INSERT INTO tasks_fts(rowid, task_id, title, statement)
    VALUES (new.rowid, new.id, new.title, new.statement);
END;

CREATE TRIGGER IF NOT EXISTS tasks_au AFTER UPDATE OF title, statement ON tasks BEGIN
    UPDATE tasks_fts
    SET title = new.title, statement = new.statement
    WHERE rowid = new.rowid;
END;

CREATE TRIGGER IF NOT EXISTS tasks_ad AFTER DELETE ON tasks BEGIN
    DELETE FROM tasks_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS reports_ai AFTER INSERT ON reports BEGIN
    INSERT INTO reports_fts(rowid, report_id, task_id, title, content)
    VALUES (new.rowid, new.id, new.task_id, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS reports_ad AFTER DELETE ON reports BEGIN
    DELETE FROM reports_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS comments_ai AFTER INSERT ON comments BEGIN
    INSERT INTO comments_fts(rowid, comment_id, task_id, body)
    VALUES (new.rowid, new.id, new.task_id, new.body);
END;

CREATE TRIGGER IF NOT EXISTS comments_ad AFTER DELETE ON comments BEGIN
    DELETE FROM comments_fts WHERE rowid = old.rowid;
END;
