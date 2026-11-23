use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::Stage;

use super::error::StoreError;

const STAGE_COLUMNS: &str = "id, task_id, role, round, started_at, finished_at, \
     end_reason, verdict, seconds, note, last_activity_at";

fn map_stage(row: &rusqlite::Row<'_>) -> rusqlite::Result<Stage> {
    Ok(Stage {
        id: row.get(0)?,
        task_id: row.get(1)?,
        role: row.get(2)?,
        round: row.get(3)?,
        started_at: row.get(4)?,
        finished_at: row.get(5)?,
        end_reason: row.get(6)?,
        verdict: row.get(7)?,
        seconds: row.get(8)?,
        note: row.get(9)?,
        last_activity_at: row.get(10)?,
    })
}

pub fn open_stage(conn: &Connection, task_id: &str) -> Result<Option<Stage>, StoreError> {
    let sql = format!(
        "SELECT {STAGE_COLUMNS} FROM stages
         WHERE task_id = ?1 AND finished_at IS NULL"
    );
    Ok(conn.query_row(&sql, [task_id], map_stage).optional()?)
}

pub fn next_round(conn: &Connection, task_id: &str, role: &str) -> Result<i64, StoreError> {
    let round: i64 = conn.query_row(
        "SELECT COALESCE(MAX(round), 0) + 1
         FROM stages WHERE task_id = ?1 AND role = ?2",
        params![task_id, role],
        |row| row.get(0),
    )?;
    Ok(round)
}

pub fn insert_stage(
    conn: &Connection,
    task_id: &str,
    role: &str,
    round: i64,
    started_at: &str,
) -> Result<Stage, StoreError> {
    let id = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO stages
         (id, task_id, role, round, started_at, finished_at, end_reason,
          verdict, seconds, note, last_activity_at)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, '', NULL, '', ?5)",
        params![id, task_id, role, round, started_at],
    )?;
    get_stage(conn, &id)
}

pub fn get_stage(conn: &Connection, stage_id: &str) -> Result<Stage, StoreError> {
    let sql = format!("SELECT {STAGE_COLUMNS} FROM stages WHERE id = ?1");
    conn.query_row(&sql, [stage_id], map_stage)
        .optional()?
        .ok_or(StoreError::NotFound)
}

/// Отмечает, что в этапе была работа. Для признака жизни в карточке.
pub fn touch_stage(conn: &Connection, stage_id: &str, at: &str) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE stages SET last_activity_at = ?2 WHERE id = ?1",
        params![stage_id, at],
    )?;
    Ok(())
}

/// Отмечает активность в открытом этапе задачи, если он есть.
pub fn touch_open_stage(conn: &Connection, task_id: &str, at: &str) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE stages SET last_activity_at = ?2
         WHERE task_id = ?1 AND finished_at IS NULL",
        params![task_id, at],
    )?;
    Ok(())
}

/// Последний этап задачи: открытый или завершённый.
pub fn latest_stage(conn: &Connection, task_id: &str) -> Result<Option<Stage>, StoreError> {
    let sql = format!(
        "SELECT {STAGE_COLUMNS} FROM stages
         WHERE task_id = ?1
         ORDER BY started_at DESC, id DESC LIMIT 1"
    );
    Ok(conn.query_row(&sql, [task_id], map_stage).optional()?)
}

/// Дописывает заметку в этап и обновляет признак активности.
pub fn append_note(
    conn: &Connection,
    stage_id: &str,
    text: &str,
    at: &str,
) -> Result<Stage, StoreError> {
    let current = get_stage(conn, stage_id)?;
    let note = if current.note.trim().is_empty() {
        text.to_string()
    } else {
        format!("{}\n{}", current.note.trim_end(), text)
    };
    conn.execute(
        "UPDATE stages SET note = ?2, last_activity_at = ?3 WHERE id = ?1",
        params![stage_id, note, at],
    )?;
    get_stage(conn, stage_id)
}

#[allow(clippy::too_many_arguments)]
pub fn finish_stage(
    conn: &Connection,
    stage_id: &str,
    finished_at: &str,
    end_reason: &str,
    verdict: &str,
    seconds: Option<i64>,
    note: &str,
) -> Result<Stage, StoreError> {
    conn.execute(
        "UPDATE stages
         SET finished_at = ?2, end_reason = ?3, verdict = ?4,
             seconds = ?5, note = ?6, last_activity_at = ?2
         WHERE id = ?1 AND finished_at IS NULL",
        params![stage_id, finished_at, end_reason, verdict, seconds, note],
    )?;
    get_stage(conn, stage_id)
}

pub fn latest_finished_stage(
    conn: &Connection,
    task_id: &str,
    role: &str,
) -> Result<Option<Stage>, StoreError> {
    let sql = format!(
        "SELECT {STAGE_COLUMNS} FROM stages
         WHERE task_id = ?1 AND role = ?2 AND finished_at IS NOT NULL
         ORDER BY round DESC LIMIT 1"
    );
    Ok(conn
        .query_row(&sql, params![task_id, role], map_stage)
        .optional()?)
}

pub fn stage_by_role_round(
    conn: &Connection,
    task_id: &str,
    role: &str,
    round: i64,
) -> Result<Option<Stage>, StoreError> {
    let sql = format!(
        "SELECT {STAGE_COLUMNS} FROM stages
         WHERE task_id = ?1 AND role = ?2 AND round = ?3"
    );
    Ok(conn
        .query_row(&sql, params![task_id, role, round], map_stage)
        .optional()?)
}

pub fn list_open_stages(conn: &Connection) -> Result<Vec<Stage>, StoreError> {
    let sql = format!(
        "SELECT {STAGE_COLUMNS} FROM stages
         WHERE finished_at IS NULL ORDER BY started_at"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_stage)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn count_open_stages(conn: &Connection) -> Result<i64, StoreError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stages WHERE finished_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}
