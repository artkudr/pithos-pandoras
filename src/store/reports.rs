use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::report::preview;
use crate::domain::{Report, ReportPreview};

use super::error::StoreError;

const REPORT_COLUMNS: &str = "id, task_id, stage_id, actor, kind, title, content, created_at";
const REPORT_COLUMNS_R: &str =
    "r.id, r.task_id, r.stage_id, r.actor, r.kind, r.title, r.content, r.created_at";

fn map_report(row: &rusqlite::Row<'_>) -> rusqlite::Result<Report> {
    Ok(Report {
        id: row.get(0)?,
        task_id: row.get(1)?,
        stage_id: row.get(2)?,
        actor: row.get(3)?,
        kind: row.get(4)?,
        title: row.get(5)?,
        content: row.get(6)?,
        created_at: row.get(7)?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn insert_report(
    conn: &Connection,
    task_id: &str,
    stage_id: Option<&str>,
    actor: Option<&str>,
    kind: &str,
    title: &str,
    content: &str,
    created_at: &str,
) -> Result<Report, StoreError> {
    let id = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO reports
         (id, task_id, stage_id, actor, kind, title, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, task_id, stage_id, actor, kind, title, content, created_at],
    )?;
    get_report(conn, &id)
}

pub fn get_report(conn: &Connection, report_id: &str) -> Result<Report, StoreError> {
    let sql = format!("SELECT {REPORT_COLUMNS} FROM reports WHERE id = ?1");
    conn.query_row(&sql, [report_id], map_report)
        .optional()?
        .ok_or(StoreError::NotFound)
}

pub fn list_report_previews(
    conn: &Connection,
    task_id: &str,
) -> Result<Vec<ReportPreview>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, stage_id, actor, kind, title, content, created_at
         FROM reports WHERE task_id = ?1
         ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map([task_id], |row| {
        let content: String = row.get(6)?;
        Ok(ReportPreview {
            id: row.get(0)?,
            task_id: row.get(1)?,
            stage_id: row.get(2)?,
            actor: row.get(3)?,
            kind: row.get(4)?,
            title: row.get(5)?,
            preview: preview(&content),
            created_at: row.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_reports_with_role(
    conn: &Connection,
    task_id: &str,
) -> Result<Vec<(Report, String)>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.task_id, r.stage_id, r.actor, r.kind, r.title, r.content, r.created_at,
                COALESCE(s.role, r.actor, '')
         FROM reports r
         LEFT JOIN stages s ON s.id = r.stage_id
         WHERE r.task_id = ?1
         ORDER BY r.created_at, r.id",
    )?;
    let rows = stmt.query_map([task_id], |row| {
        let report = Report {
            id: row.get(0)?,
            task_id: row.get(1)?,
            stage_id: row.get(2)?,
            actor: row.get(3)?,
            kind: row.get(4)?,
            title: row.get(5)?,
            content: row.get(6)?,
            created_at: row.get(7)?,
        };
        let role: String = row.get(8)?;
        Ok((report, role))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn report_for_stage_kind(
    conn: &Connection,
    stage_id: &str,
    kind: &str,
) -> Result<Option<Report>, StoreError> {
    let sql = format!("SELECT {REPORT_COLUMNS} FROM reports WHERE stage_id = ?1 AND kind = ?2");
    Ok(conn
        .query_row(&sql, params![stage_id, kind], map_report)
        .optional()?)
}

/// Находит отчёт для правила входа.
///
/// `round` — значение из правила: `latest`, `same`, `previous`, `return`.
/// Для `previous` берётся круг той же роли, для остальных — роль `from_role`.
pub fn find_report_for_rule(
    conn: &Connection,
    task_id: &str,
    from_role: &str,
    kind: &str,
    round: &str,
    current_round: i64,
    current_role: &str,
) -> Result<Option<(i64, Report)>, StoreError> {
    let (sql, role, round_value) = match round {
        "latest" => (
            format!(
                "SELECT s.round, {REPORT_COLUMNS_R}
                 FROM stages s
                 JOIN reports r ON r.stage_id = s.id
                 WHERE s.task_id = ?1 AND s.role = ?2
                   AND s.finished_at IS NOT NULL AND r.kind = ?3
                 ORDER BY s.round DESC LIMIT 1"
            ),
            from_role,
            None,
        ),
        "same" => (
            format!(
                "SELECT s.round, {REPORT_COLUMNS_R}
                 FROM stages s
                 JOIN reports r ON r.stage_id = s.id
                 WHERE s.task_id = ?1 AND s.role = ?2
                   AND s.round = ?4 AND r.kind = ?3
                 LIMIT 1"
            ),
            from_role,
            Some(current_round),
        ),
        "previous" => (
            format!(
                "SELECT s.round, {REPORT_COLUMNS_R}
                 FROM stages s
                 JOIN reports r ON r.stage_id = s.id
                 WHERE s.task_id = ?1 AND s.role = ?2
                   AND s.round = ?4 AND r.kind = ?3
                 LIMIT 1"
            ),
            current_role,
            Some(current_round - 1),
        ),
        "return" => return find_return_report(conn, task_id, from_role),
        other => {
            return Err(StoreError::Invalid(format!(
                "неизвестное правило круга: {other}"
            )))
        }
    };

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = match round_value {
        Some(value) => stmt.query(params![task_id, role, kind, value])?,
        None => stmt.query(params![task_id, role, kind])?,
    };
    let row = rows.next()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stage_round: i64 = row.get(0)?;
    let report = Report {
        id: row.get(1)?,
        task_id: row.get(2)?,
        stage_id: row.get(3)?,
        actor: row.get(4)?,
        kind: row.get(5)?,
        title: row.get(6)?,
        content: row.get(7)?,
        created_at: row.get(8)?,
    };
    Ok(Some((stage_round, report)))
}

fn find_return_report(
    conn: &Connection,
    task_id: &str,
    from_role: &str,
) -> Result<Option<(i64, Report)>, StoreError> {
    let row = conn
        .query_row(
            "SELECT e.payload, s.round
             FROM events e
             JOIN stages s ON s.id = e.stage_id
             WHERE e.task_id = ?1 AND e.kind = 'stage_returned' AND s.role = ?2
             ORDER BY e.seq DESC LIMIT 1",
            params![task_id, from_role],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let Some((payload, round)) = row else {
        return Ok(None);
    };
    let value: serde_json::Value = serde_json::from_str(&payload)?;
    let report_id = value
        .get("report_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| StoreError::Invalid("stage_returned без report_id".to_string()))?;
    let report = get_report(conn, report_id)?;
    Ok(Some((round, report)))
}
