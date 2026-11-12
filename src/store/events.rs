use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::Event;

use super::db::now_rfc3339;
use super::error::StoreError;

const EVENT_COLUMNS: &str = "id, uuid, task_id, seq, at, kind, stage_id, actor, payload, origin";

fn map_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get(0)?,
        uuid: row.get(1)?,
        task_id: row.get(2)?,
        seq: row.get(3)?,
        at: row.get(4)?,
        kind: row.get(5)?,
        stage_id: row.get(6)?,
        actor: row.get(7)?,
        payload: row.get(8)?,
        origin: row.get(9)?,
    })
}

pub fn next_seq(conn: &Connection, task_id: &str) -> Result<i64, StoreError> {
    let seq: i64 = conn.query_row(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM events WHERE task_id = ?1",
        [task_id],
        |row| row.get(0),
    )?;
    Ok(seq)
}

pub fn insert_event(
    conn: &Connection,
    task_id: &str,
    kind: &str,
    stage_id: Option<&str>,
    actor: Option<&str>,
    payload: &str,
    origin: &str,
) -> Result<Event, StoreError> {
    let id = Uuid::now_v7().to_string();
    let seq = next_seq(conn, task_id)?;
    let at = now_rfc3339();
    conn.execute(
        "INSERT INTO events
         (uuid, task_id, seq, at, kind, stage_id, actor, payload, origin)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, task_id, seq, at, kind, stage_id, actor, payload, origin],
    )?;
    let rowid = conn.last_insert_rowid();
    get_event(conn, rowid)
}

pub fn get_event(conn: &Connection, event_id: i64) -> Result<Event, StoreError> {
    let sql = format!("SELECT {EVENT_COLUMNS} FROM events WHERE id = ?1");
    conn.query_row(&sql, [event_id], map_event)
        .optional()?
        .ok_or(StoreError::NotFound)
}

pub fn list_events(conn: &Connection, task_id: &str) -> Result<Vec<Event>, StoreError> {
    let sql = format!("SELECT {EVENT_COLUMNS} FROM events WHERE task_id = ?1 ORDER BY seq");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([task_id], map_event)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
