use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::{DraftBlock, ReportDraft};

use super::error::StoreError;

const DRAFT_COLUMNS: &str = "id, task_id, stage_id, actor, kind, title, revision, \
     created_at, updated_at, published_report_id";

fn map_draft(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReportDraft> {
    Ok(ReportDraft {
        draft_id: row.get(0)?,
        task_id: row.get(1)?,
        stage_id: row.get(2)?,
        actor: row.get(3)?,
        kind: row.get(4)?,
        title: row.get(5)?,
        revision: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        published_report_id: row.get(9)?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn insert_draft(
    conn: &Connection,
    task_id: &str,
    stage_id: &str,
    actor: &str,
    kind: &str,
    title: &str,
    created_at: &str,
) -> Result<ReportDraft, StoreError> {
    let id = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO report_drafts
         (id, task_id, stage_id, actor, kind, title, revision, created_at, updated_at,
          published_report_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?7, NULL)",
        params![id, task_id, stage_id, actor, kind, title, created_at],
    )?;
    get_draft(conn, &id)
}

pub fn get_draft(conn: &Connection, draft_id: &str) -> Result<ReportDraft, StoreError> {
    let sql = format!("SELECT {DRAFT_COLUMNS} FROM report_drafts WHERE id = ?1");
    conn.query_row(&sql, [draft_id], map_draft)
        .optional()?
        .ok_or(StoreError::NotFound)
}

pub fn find_open_draft(
    conn: &Connection,
    task_id: &str,
    stage_id: &str,
    actor: &str,
    kind: &str,
) -> Result<Option<ReportDraft>, StoreError> {
    let sql = format!(
        "SELECT {DRAFT_COLUMNS} FROM report_drafts
         WHERE task_id = ?1 AND stage_id = ?2 AND actor = ?3 AND kind = ?4
           AND published_report_id IS NULL
         ORDER BY created_at DESC LIMIT 1"
    );
    Ok(conn
        .query_row(&sql, params![task_id, stage_id, actor, kind], map_draft)
        .optional()?)
}

pub fn list_open_drafts(conn: &Connection, task_id: &str) -> Result<Vec<ReportDraft>, StoreError> {
    let sql = format!(
        "SELECT {DRAFT_COLUMNS} FROM report_drafts
         WHERE task_id = ?1 AND published_report_id IS NULL
         ORDER BY updated_at DESC, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([task_id], map_draft)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_blocks(conn: &Connection, draft_id: &str) -> Result<Vec<DraftBlock>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT key, title, body, sort
         FROM report_draft_blocks
         WHERE draft_id = ?1
         ORDER BY sort, key",
    )?;
    let rows = stmt.query_map([draft_id], |row| {
        Ok(DraftBlock {
            key: row.get(0)?,
            title: row.get(1)?,
            body: row.get(2)?,
            sort: row.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn next_sort(conn: &Connection, draft_id: &str) -> Result<i64, StoreError> {
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort), 0) + 1 FROM report_draft_blocks WHERE draft_id = ?1",
        [draft_id],
        |row| row.get(0),
    )?;
    Ok(next)
}

pub fn block_sort(conn: &Connection, draft_id: &str, key: &str) -> Result<Option<i64>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT sort FROM report_draft_blocks WHERE draft_id = ?1 AND key = ?2",
            params![draft_id, key],
            |row| row.get(0),
        )
        .optional()?)
}

#[allow(clippy::too_many_arguments)]
pub fn upsert_block(
    conn: &Connection,
    draft_id: &str,
    key: &str,
    title: &str,
    body: &str,
    sort: i64,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO report_draft_blocks (draft_id, key, title, body, sort)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(draft_id, key) DO UPDATE SET
             title = excluded.title,
             body = excluded.body,
             sort = excluded.sort",
        params![draft_id, key, title, body, sort],
    )?;
    Ok(())
}

pub fn bump_revision(
    conn: &Connection,
    draft_id: &str,
    updated_at: &str,
) -> Result<i64, StoreError> {
    let revision = conn.query_row(
        "UPDATE report_drafts
         SET revision = revision + 1, updated_at = ?2
         WHERE id = ?1
         RETURNING revision",
        params![draft_id, updated_at],
        |row| row.get(0),
    )?;
    Ok(revision)
}

pub fn mark_published(
    conn: &Connection,
    draft_id: &str,
    report_id: &str,
    updated_at: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE report_drafts
         SET published_report_id = ?2, updated_at = ?3
         WHERE id = ?1",
        params![draft_id, report_id, updated_at],
    )?;
    Ok(())
}

pub fn delete_draft(conn: &Connection, draft_id: &str) -> Result<(), StoreError> {
    conn.execute("DELETE FROM report_drafts WHERE id = ?1", [draft_id])?;
    Ok(())
}
