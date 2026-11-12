use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::{Role, Status, Workspace, WorkspaceView};

use super::db::now_rfc3339;
use super::error::StoreError;

const WORKSPACE_COLUMNS: &str =
    "id, name, root_path, tasks_dir, folder_pattern, config_json, created_at";

fn map_workspace(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        tasks_dir: row.get(3)?,
        folder_pattern: row.get(4)?,
        config_json: row.get(5)?,
        created_at: row.get(6)?,
    })
}

pub fn get_workspace(conn: &Connection, name: &str) -> Result<Workspace, StoreError> {
    let sql =
        format!("SELECT {WORKSPACE_COLUMNS} FROM workspaces WHERE name = ?1 AND archived = 0");
    conn.query_row(&sql, [name], map_workspace)
        .optional()?
        .ok_or(StoreError::NotFound)
}

pub fn list_workspace_names(conn: &Connection) -> Result<Vec<String>, StoreError> {
    let mut stmt = conn.prepare("SELECT name FROM workspaces WHERE archived = 0 ORDER BY name")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Статус проекта вместе с числом задач в нём. Нужен списку проектов.
#[derive(Debug, Clone)]
pub struct StatusTaskCount {
    pub workspace_id: String,
    pub id: String,
    pub title: String,
    pub color: Option<String>,
    pub count: i64,
}

pub fn list_status_counts(conn: &Connection) -> Result<Vec<StatusTaskCount>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT s.workspace_id, s.id, s.title, s.color, COUNT(t.id)
         FROM workspace_statuses s
         LEFT JOIN tasks t
           ON t.workspace_id = s.workspace_id AND t.status_id = s.id
         GROUP BY s.workspace_id, s.id, s.title, s.color, s.sort
         ORDER BY s.sort, s.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(StatusTaskCount {
            workspace_id: row.get(0)?,
            id: row.get(1)?,
            title: row.get(2)?,
            color: row.get(3)?,
            count: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_roles(conn: &Connection, workspace_id: &str) -> Result<Vec<Role>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT name, color, sort FROM workspace_roles
         WHERE workspace_id = ?1 ORDER BY sort, name",
    )?;
    let rows = stmt.query_map([workspace_id], |row| {
        Ok(Role {
            name: row.get(0)?,
            color: row.get(1)?,
            sort: row.get(2)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_statuses(conn: &Connection, workspace_id: &str) -> Result<Vec<Status>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, color, sort FROM workspace_statuses
         WHERE workspace_id = ?1 ORDER BY sort, id",
    )?;
    let rows = stmt.query_map([workspace_id], |row| {
        Ok(Status {
            id: row.get(0)?,
            title: row.get(1)?,
            kind: row.get(2)?,
            color: row.get(3)?,
            sort: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn workspace_view(conn: &Connection, name: &str) -> Result<WorkspaceView, StoreError> {
    let workspace = get_workspace(conn, name)?;
    let roles = list_roles(conn, &workspace.id)?;
    let statuses = list_statuses(conn, &workspace.id)?;
    Ok(WorkspaceView {
        workspace,
        roles,
        statuses,
    })
}

pub fn list_workspaces(conn: &Connection) -> Result<Vec<Workspace>, StoreError> {
    let sql =
        format!("SELECT {WORKSPACE_COLUMNS} FROM workspaces WHERE archived = 0 ORDER BY name");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_workspace)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn add_role(conn: &Connection, workspace_id: &str, name: &str) -> Result<(), StoreError> {
    let sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort), 0) + 10 FROM workspace_roles WHERE workspace_id = ?1",
        [workspace_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO workspace_roles
         (workspace_id, name, color, sort)
         VALUES (?1, ?2, NULL, ?3)",
        params![workspace_id, name, sort],
    )?;
    Ok(())
}

pub fn upsert_status(
    conn: &Connection,
    workspace_id: &str,
    id: &str,
    title: &str,
    kind: &str,
    sort: i64,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO workspace_statuses
         (workspace_id, id, title, kind, color, sort)
         VALUES (?1, ?2, ?3, ?4, NULL, ?5)
         ON CONFLICT(workspace_id, id) DO UPDATE SET
             title = excluded.title,
             kind = excluded.kind,
             sort = excluded.sort",
        params![workspace_id, id, title, kind, sort],
    )?;
    Ok(())
}

pub fn update_config(
    conn: &Connection,
    workspace_id: &str,
    config_json: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE workspaces SET config_json = ?2 WHERE id = ?1",
        params![workspace_id, config_json],
    )?;
    Ok(())
}

pub fn update_paths(
    conn: &Connection,
    workspace_id: &str,
    root_path: &str,
    tasks_dir: &str,
    folder_pattern: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE workspaces
         SET root_path = ?2, tasks_dir = ?3, folder_pattern = ?4
         WHERE id = ?1",
        params![workspace_id, root_path, tasks_dir, folder_pattern],
    )?;
    Ok(())
}

pub fn init_workspace(
    conn: &mut Connection,
    name: &str,
    root_path: &str,
    tasks_dir: &str,
    folder_pattern: &str,
    roles: &[String],
    config_json: &str,
) -> Result<Workspace, StoreError> {
    let now = now_rfc3339();
    let tx = conn.transaction()?;

    let existing: Option<String> = tx
        .query_row("SELECT id FROM workspaces WHERE name = ?1", [name], |row| {
            row.get(0)
        })
        .optional()?;

    let workspace_id = match existing {
        Some(id) => id,
        None => {
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO workspaces
                 (id, name, root_path, tasks_dir, folder_pattern, config_json, created_at, archived)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                params![
                    id,
                    name,
                    root_path,
                    tasks_dir,
                    folder_pattern,
                    config_json,
                    now
                ],
            )?;
            id
        }
    };

    tx.execute(
        "INSERT OR IGNORE INTO workspace_counters(workspace_id, next_number) VALUES (?1, 1)",
        [&workspace_id],
    )?;

    let statuses = [
        ("new", "Новая", "active", "#5c7cfa", 10_i64),
        ("active", "В работе", "active", "#1c7ed6", 20),
        ("waiting", "Ожидание", "waiting", "#e8890c", 30),
        ("blocked", "Остановлена", "blocked", "#e03131", 40),
        ("done", "Закрыта", "done", "#2f9e44", 50),
    ];
    for (id, title, kind, color, sort) in statuses {
        tx.execute(
            "INSERT OR IGNORE INTO workspace_statuses
             (workspace_id, id, title, kind, color, sort)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![workspace_id, id, title, kind, color, sort],
        )?;
    }

    for (index, role) in roles.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO workspace_roles
             (workspace_id, name, color, sort)
             VALUES (?1, ?2, NULL, ?3)",
            params![workspace_id, role, index as i64],
        )?;
    }

    tx.commit()?;
    get_workspace(conn, name)
}
