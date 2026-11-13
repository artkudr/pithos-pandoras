use std::path::{Component, Path};

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::{Project, ProjectView, Role, Status};

use super::db::now_rfc3339;
use super::error::StoreError;

const PROJECT_COLUMNS: &str =
    "id, name, root_path, tasks_dir, folder_pattern, config_json, created_at";

fn map_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        tasks_dir: row.get(3)?,
        folder_pattern: row.get(4)?,
        config_json: row.get(5)?,
        created_at: row.get(6)?,
    })
}

pub fn get_project(conn: &Connection, name: &str) -> Result<Project, StoreError> {
    let sql = format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE name = ?1 AND archived = 0");
    conn.query_row(&sql, [name], map_project)
        .optional()?
        .ok_or(StoreError::NotFound)
}

pub fn list_project_names(conn: &Connection) -> Result<Vec<String>, StoreError> {
    let mut stmt = conn.prepare("SELECT name FROM projects WHERE archived = 0 ORDER BY name")?;
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
    pub project_id: String,
    pub id: String,
    pub title: String,
    pub color: Option<String>,
    pub count: i64,
}

pub fn list_status_counts(conn: &Connection) -> Result<Vec<StatusTaskCount>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT s.project_id, s.id, s.title, s.color, COUNT(t.id)
         FROM project_statuses s
         LEFT JOIN tasks t
           ON t.project_id = s.project_id AND t.status_id = s.id
         GROUP BY s.project_id, s.id, s.title, s.color, s.sort
         ORDER BY s.sort, s.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(StatusTaskCount {
            project_id: row.get(0)?,
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

pub fn list_roles(conn: &Connection, project_id: &str) -> Result<Vec<Role>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT name, color, sort FROM project_roles
         WHERE project_id = ?1 ORDER BY sort, name",
    )?;
    let rows = stmt.query_map([project_id], |row| {
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

pub fn list_statuses(conn: &Connection, project_id: &str) -> Result<Vec<Status>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, title, kind, color, sort FROM project_statuses
         WHERE project_id = ?1 ORDER BY sort, id",
    )?;
    let rows = stmt.query_map([project_id], |row| {
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

pub fn project_view(conn: &Connection, name: &str) -> Result<ProjectView, StoreError> {
    let project = get_project(conn, name)?;
    let roles = list_roles(conn, &project.id)?;
    let statuses = list_statuses(conn, &project.id)?;
    Ok(ProjectView {
        project,
        roles,
        statuses,
    })
}

pub fn list_projects(conn: &Connection) -> Result<Vec<Project>, StoreError> {
    let sql = format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE archived = 0 ORDER BY name");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_project)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn add_role(
    conn: &Connection,
    project_id: &str,
    name: &str,
    color: Option<&str>,
) -> Result<(), StoreError> {
    let sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort), 0) + 10 FROM project_roles WHERE project_id = ?1",
        [project_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO project_roles
         (project_id, name, color, sort)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(project_id, name) DO UPDATE SET color = excluded.color",
        params![project_id, name, color, sort],
    )?;
    Ok(())
}

pub fn update_role(
    conn: &Connection,
    project_id: &str,
    old_name: &str,
    new_name: &str,
    color: Option<&str>,
) -> Result<(), StoreError> {
    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM project_roles WHERE project_id = ?1 AND name = ?2",
        params![project_id, new_name],
        |row| row.get(0),
    )?;
    if new_name != old_name && existing > 0 {
        return Err(StoreError::Conflict(String::from(
            "роль с таким именем уже есть",
        )));
    }
    let changed = conn.execute(
        "UPDATE project_roles SET name = ?3, color = ?4
         WHERE project_id = ?1 AND name = ?2",
        params![project_id, old_name, new_name, color],
    )?;
    if changed == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

pub fn delete_role(conn: &Connection, project_id: &str, name: &str) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM project_roles WHERE project_id = ?1 AND name = ?2",
        params![project_id, name],
    )?;
    Ok(())
}

pub fn upsert_status(
    conn: &Connection,
    project_id: &str,
    id: &str,
    title: &str,
    kind: &str,
    color: Option<&str>,
    sort: i64,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO project_statuses
         (project_id, id, title, kind, color, sort)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(project_id, id) DO UPDATE SET
             title = excluded.title,
             kind = excluded.kind,
             color = COALESCE(excluded.color, project_statuses.color),
             sort = excluded.sort",
        params![project_id, id, title, kind, color, sort],
    )?;
    Ok(())
}

pub fn update_config(
    conn: &Connection,
    project_id: &str,
    config_json: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE projects SET config_json = ?2 WHERE id = ?1",
        params![project_id, config_json],
    )?;
    Ok(())
}

pub fn update_paths(
    conn: &Connection,
    project_id: &str,
    root_path: &str,
    tasks_dir: &str,
    folder_pattern: &str,
) -> Result<(), StoreError> {
    let folder_pattern = folder_pattern.trim();
    let root_path = normalize_root_path(root_path)?;
    let tasks_dir = normalize_tasks_dir(tasks_dir)?;
    validate_folder_pattern(folder_pattern)?;
    ensure_paths_free(conn, Some(project_id), &root_path, &tasks_dir)?;

    conn.execute(
        "UPDATE projects
         SET root_path = ?2, tasks_dir = ?3, folder_pattern = ?4
         WHERE id = ?1",
        params![project_id, root_path, tasks_dir, folder_pattern],
    )?;
    Ok(())
}

/// Имя проекта попадает в адрес страницы, поэтому `/`, `\`, `?` и `#` нельзя.
fn validate_project_name(name: &str) -> Result<(), StoreError> {
    if name.is_empty() {
        return Err(StoreError::Conflict(String::from(
            "Имя проекта не может быть пустым",
        )));
    }
    if name == "." || name == ".." {
        return Err(StoreError::Conflict(String::from(
            "Имя проекта не может быть `.` или `..`",
        )));
    }
    if name
        .chars()
        .any(|ch| ch == '/' || ch == '\\' || ch == '?' || ch == '#' || ch.is_control())
    {
        return Err(StoreError::Conflict(String::from(
            "В имени проекта нельзя использовать `/`, `\\`, `?`, `#` и управляющие символы",
        )));
    }
    Ok(())
}

/// Корень проекта храним абсолютным: иначе он зависит от рабочего каталога.
fn normalize_root_path(value: &str) -> Result<String, StoreError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(StoreError::Conflict(String::from(
            "Корень проекта не может быть пустым",
        )));
    }
    let path = Path::new(value);
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let canonical = std::fs::canonicalize(&absolute).unwrap_or(absolute);
    Ok(canonical
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_string())
}

/// Папка задач — только внутри корня проекта.
fn normalize_tasks_dir(value: &str) -> Result<String, StoreError> {
    let trimmed = value.trim().trim_end_matches(['/', '\\']);
    let path = Path::new(trimmed);
    if trimmed.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::CurDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(StoreError::Conflict(String::from(
            "Папка задач должна быть относительной и не выходить за корень проекта",
        )));
    }
    Ok(trimmed.replace('\\', "/"))
}

fn validate_folder_pattern(pattern: &str) -> Result<(), StoreError> {
    if pattern.trim().is_empty() || !pattern.contains("{number}") {
        return Err(StoreError::Conflict(String::from(
            "Шаблон папки должен содержать `{number}`, иначе задачи столкнутся",
        )));
    }
    let path = Path::new(pattern);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(StoreError::Conflict(String::from(
            "Шаблон папки не должен выходить за папку задач",
        )));
    }
    Ok(())
}

/// Один и тот же каталог задач — только у одного проекта.
///
/// Сравниваем не `root_path` и `tasks_dir` по отдельности, а итоговый путь:
/// `корень/Задачи/Проект` и `корень/Проект/Задачи` могут совпасть.
fn ensure_paths_free(
    conn: &Connection,
    except_project_id: Option<&str>,
    root_path: &str,
    tasks_dir: &str,
) -> Result<(), StoreError> {
    let target = path_key(&Path::new(root_path).join(tasks_dir));
    let mut stmt = conn.prepare(
        "SELECT name, root_path, tasks_dir FROM projects
         WHERE archived = 0 AND id != ?1",
    )?;
    let rows = stmt.query_map([except_project_id.unwrap_or("")], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (name, root, tasks) = row?;
        let root = normalize_root_path(&root).unwrap_or(root);
        let existing = path_key(&Path::new(&root).join(tasks.replace('\\', "/")));
        if existing == target {
            return Err(StoreError::Conflict(format!(
                "Папка задач уже занята проектом `{name}`"
            )));
        }
    }
    Ok(())
}

fn path_key(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

pub fn init_project(
    conn: &mut Connection,
    name: &str,
    root_path: &str,
    tasks_dir: &str,
    folder_pattern: &str,
    roles: &[(String, Option<String>)],
    config_json: &str,
) -> Result<Project, StoreError> {
    let now = now_rfc3339();
    let name = name.trim();
    validate_project_name(name)?;
    let folder_pattern = folder_pattern.trim();
    validate_folder_pattern(folder_pattern)?;
    let root_path = normalize_root_path(root_path)?;
    let tasks_dir = normalize_tasks_dir(tasks_dir)?;

    let tx = conn.transaction()?;

    let existing: Option<String> = tx
        .query_row("SELECT id FROM projects WHERE name = ?1", [name], |row| {
            row.get(0)
        })
        .optional()?;
    ensure_paths_free(&tx, existing.as_deref(), &root_path, &tasks_dir)?;

    let project_id = match existing {
        Some(id) => id,
        None => {
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO projects
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
        "INSERT OR IGNORE INTO project_counters(project_id, next_number) VALUES (?1, 1)",
        [&project_id],
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
            "INSERT OR IGNORE INTO project_statuses
             (project_id, id, title, kind, color, sort)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![project_id, id, title, kind, color, sort],
        )?;
    }

    for (index, (role, color)) in roles.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO project_roles
             (project_id, name, color, sort)
             VALUES (?1, ?2, ?3, ?4)",
            params![project_id, role, color, index as i64],
        )?;
    }

    tx.commit()?;
    get_project(conn, name)
}
