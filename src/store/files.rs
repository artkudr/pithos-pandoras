use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::FileRecord;

use super::error::StoreError;

const FILE_COLUMNS: &str =
    "id, task_id, stage_id, report_id, actor, kind, title, comment, path, mime, size, created_at";

fn map_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        stage_id: row.get(2)?,
        report_id: row.get(3)?,
        actor: row.get(4)?,
        kind: row.get(5)?,
        title: row.get(6)?,
        comment: row.get(7)?,
        path: row.get(8)?,
        mime: row.get(9)?,
        size: row.get(10)?,
        created_at: row.get(11)?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn insert_file(
    conn: &Connection,
    task_id: &str,
    stage_id: Option<&str>,
    report_id: Option<&str>,
    actor: &str,
    kind: &str,
    title: &str,
    comment: &str,
    path: &str,
    mime: Option<&str>,
    size: Option<i64>,
    created_at: &str,
) -> Result<FileRecord, StoreError> {
    let id = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO files
         (id, task_id, stage_id, report_id, actor, kind, title, comment, path, mime, size, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id, task_id, stage_id, report_id, actor, kind, title, comment, path, mime, size,
            created_at
        ],
    )?;
    get_file(conn, &id)
}

pub fn get_file(conn: &Connection, file_id: &str) -> Result<FileRecord, StoreError> {
    let sql = format!("SELECT {FILE_COLUMNS} FROM files WHERE id = ?1");
    conn.query_row(&sql, [file_id], map_file)
        .optional()?
        .ok_or(StoreError::NotFound)
}

pub fn list_files(conn: &Connection, task_id: &str) -> Result<Vec<FileRecord>, StoreError> {
    let sql =
        format!("SELECT {FILE_COLUMNS} FROM files WHERE task_id = ?1 ORDER BY created_at, id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([task_id], map_file)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_all_files(conn: &Connection) -> Result<Vec<FileRecord>, StoreError> {
    let sql = format!("SELECT {FILE_COLUMNS} FROM files ORDER BY created_at, id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_file)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
