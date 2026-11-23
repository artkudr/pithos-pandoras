use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::{Status, Task, TaskListItem};

use super::db::now_rfc3339;
use super::error::StoreError;

const TASK_COLUMNS: &str = "id, project_id, number, folder_name, title, statement, status_id, \
     route, route_snapshot, resource, waiting_for, waiting_ref, waiting_since, \
     created_at, updated_at, closed_at, origin";

fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        project_id: row.get(1)?,
        number: row.get(2)?,
        folder_name: row.get(3)?,
        title: row.get(4)?,
        statement: row.get(5)?,
        status_id: row.get(6)?,
        route: row.get(7)?,
        route_snapshot: row.get(8)?,
        resource: row.get(9)?,
        waiting_for: row.get(10)?,
        waiting_ref: row.get(11)?,
        waiting_since: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        closed_at: row.get(15)?,
        origin: row.get(16)?,
        stage_closed: None,
    })
}

pub fn get_task_by_id(conn: &Connection, task_id: &str) -> Result<Task, StoreError> {
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1");
    conn.query_row(&sql, [task_id], map_task)
        .optional()?
        .ok_or(StoreError::NotFound)
}

pub fn get_task(conn: &Connection, project_id: &str, task_ref: &str) -> Result<Task, StoreError> {
    if let Ok(number) = task_ref.parse::<i64>() {
        let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE project_id = ?1 AND number = ?2");
        conn.query_row(&sql, params![project_id, number], map_task)
            .optional()?
            .ok_or(StoreError::NotFound)
    } else {
        let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE project_id = ?1 AND id = ?2");
        conn.query_row(&sql, params![project_id, task_ref], map_task)
            .optional()?
            .ok_or(StoreError::NotFound)
    }
}

pub fn next_number(conn: &Connection, project_id: &str) -> Result<i64, StoreError> {
    let number: i64 = conn.query_row(
        "UPDATE project_counters
         SET next_number = next_number + 1
         WHERE project_id = ?1
         RETURNING next_number - 1",
        [project_id],
        |row| row.get(0),
    )?;
    Ok(number)
}

#[allow(clippy::too_many_arguments)]
pub fn insert_task(
    conn: &Connection,
    project_id: &str,
    number: i64,
    folder_name: &str,
    title: &str,
    statement: &str,
    status_id: &str,
    route: Option<&str>,
    resource: Option<&str>,
    origin: &str,
) -> Result<Task, StoreError> {
    let id = Uuid::now_v7().to_string();
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO tasks
         (id, project_id, number, folder_name, title, statement, status_id,
          route, route_snapshot, resource, waiting_for, waiting_ref, waiting_since,
          created_at, updated_at, closed_at, origin)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, '', '', NULL, ?10, ?10, NULL, ?11)",
        params![
            id,
            project_id,
            number,
            folder_name,
            title,
            statement,
            status_id,
            route,
            resource,
            now,
            origin
        ],
    )?;
    get_task_by_id(conn, &id)
}

pub fn list_tasks(conn: &Connection, project_id: &str) -> Result<Vec<TaskListItem>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.number, t.folder_name, t.title, t.status_id,
                COALESCE(s.title, t.status_id) AS status_title,
                (SELECT st.role FROM stages st
                  WHERE st.task_id = t.id AND st.finished_at IS NULL
                  LIMIT 1) AS current_role,
                (SELECT r.color FROM stages st
                  JOIN project_roles r
                    ON r.project_id = t.project_id AND r.name = st.role
                  WHERE st.task_id = t.id AND st.finished_at IS NULL
                  LIMIT 1) AS current_role_color,
                t.waiting_for, t.updated_at
         FROM tasks t
         LEFT JOIN project_statuses s
           ON s.project_id = t.project_id AND s.id = t.status_id
         WHERE t.project_id = ?1
         ORDER BY t.number DESC",
    )?;
    let rows = stmt.query_map([project_id], |row| {
        Ok(TaskListItem {
            id: row.get(0)?,
            number: row.get(1)?,
            folder_name: row.get(2)?,
            title: row.get(3)?,
            status_id: row.get(4)?,
            status_title: row.get(5)?,
            current_role: row.get(6)?,
            current_role_color: row.get(7)?,
            waiting_for: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn update_task(
    conn: &Connection,
    task_id: &str,
    title: Option<&str>,
    statement: Option<&str>,
) -> Result<Task, StoreError> {
    let now = now_rfc3339();
    conn.execute(
        "UPDATE tasks
         SET title = COALESCE(?2, title),
             statement = COALESCE(?3, statement),
             updated_at = ?4
         WHERE id = ?1",
        params![task_id, title, statement, now],
    )?;
    get_task_by_id(conn, task_id)
}

pub fn set_timestamps(
    conn: &Connection,
    task_id: &str,
    created_at: &str,
    updated_at: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE tasks SET created_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![task_id, created_at, updated_at],
    )?;
    Ok(())
}

pub fn set_status(conn: &Connection, task_id: &str, status_id: &str) -> Result<Task, StoreError> {
    let now = now_rfc3339();
    conn.execute(
        "UPDATE tasks SET status_id = ?2, updated_at = ?3 WHERE id = ?1",
        params![task_id, status_id, now],
    )?;
    get_task_by_id(conn, task_id)
}

pub fn close_task(conn: &Connection, project_id: &str, task_id: &str) -> Result<Task, StoreError> {
    let done = status_by_kind(conn, project_id, "done")?;
    let now = now_rfc3339();
    conn.execute(
        "UPDATE tasks
         SET status_id = ?2, closed_at = ?3, updated_at = ?3,
             waiting_for = '', waiting_ref = '', waiting_since = NULL
         WHERE id = ?1",
        params![task_id, done.id, now],
    )?;
    get_task_by_id(conn, task_id)
}

pub fn set_waiting(
    conn: &Connection,
    task_id: &str,
    status_id: &str,
    waiting_for: &str,
    waiting_ref: &str,
) -> Result<Task, StoreError> {
    let now = now_rfc3339();
    conn.execute(
        "UPDATE tasks
         SET status_id = ?2, waiting_for = ?3, waiting_ref = ?4,
             waiting_since = ?5, updated_at = ?5
         WHERE id = ?1",
        params![task_id, status_id, waiting_for, waiting_ref, now],
    )?;
    get_task_by_id(conn, task_id)
}

pub fn set_blocked(conn: &Connection, task_id: &str, status_id: &str) -> Result<Task, StoreError> {
    let now = now_rfc3339();
    conn.execute(
        "UPDATE tasks
         SET status_id = ?2, waiting_for = '', waiting_ref = '',
             waiting_since = NULL, updated_at = ?3
         WHERE id = ?1",
        params![task_id, status_id, now],
    )?;
    get_task_by_id(conn, task_id)
}

pub fn clear_waiting(
    conn: &Connection,
    task_id: &str,
    status_id: &str,
) -> Result<Task, StoreError> {
    let now = now_rfc3339();
    conn.execute(
        "UPDATE tasks
         SET status_id = ?2, waiting_for = '', waiting_ref = '',
             waiting_since = NULL, updated_at = ?3
         WHERE id = ?1",
        params![task_id, status_id, now],
    )?;
    get_task_by_id(conn, task_id)
}

pub fn reopen_task(conn: &Connection, project_id: &str, task_id: &str) -> Result<Task, StoreError> {
    let active = status_by_kind(conn, project_id, "active")?;
    let now = now_rfc3339();
    conn.execute(
        "UPDATE tasks
         SET status_id = ?2, closed_at = NULL,
             waiting_for = '', waiting_ref = '', waiting_since = NULL,
             updated_at = ?3
         WHERE id = ?1",
        params![task_id, active.id, now],
    )?;
    get_task_by_id(conn, task_id)
}

pub fn status_by_kind(
    conn: &Connection,
    project_id: &str,
    kind: &str,
) -> Result<Status, StoreError> {
    conn.query_row(
        "SELECT id, title, kind, color, sort
         FROM project_statuses
         WHERE project_id = ?1 AND kind = ?2
         ORDER BY CASE WHEN id = ?2 THEN 0 ELSE 1 END, sort, id
         LIMIT 1",
        params![project_id, kind],
        |row| {
            Ok(Status {
                id: row.get(0)?,
                title: row.get(1)?,
                kind: row.get(2)?,
                color: row.get(3)?,
                sort: row.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or(StoreError::NotFound)
}

pub fn status_exists(
    conn: &Connection,
    project_id: &str,
    status_id: &str,
) -> Result<bool, StoreError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM project_statuses WHERE project_id = ?1 AND id = ?2",
        params![project_id, status_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn status_by_id(
    conn: &Connection,
    project_id: &str,
    status_id: &str,
) -> Result<Status, StoreError> {
    conn.query_row(
        "SELECT id, title, kind, color, sort
         FROM project_statuses
         WHERE project_id = ?1 AND id = ?2",
        params![project_id, status_id],
        |row| {
            Ok(Status {
                id: row.get(0)?,
                title: row.get(1)?,
                kind: row.get(2)?,
                color: row.get(3)?,
                sort: row.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or(StoreError::NotFound)
}

pub fn resource_busy(
    conn: &Connection,
    project_id: &str,
    resource: &str,
    task_id: &str,
) -> Result<bool, StoreError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM tasks t
         JOIN project_statuses s
           ON s.project_id = t.project_id AND s.id = t.status_id
         WHERE t.project_id = ?1 AND t.resource = ?2
           AND s.kind != 'done' AND t.id != ?3",
        params![project_id, resource, task_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}
