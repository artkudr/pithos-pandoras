//! Импорт старых задач из папки каркаса.

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Args;
use serde::Deserialize;

use crate::store::{db, events, files, projects, reports, tasks, Store};

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Проект, в который импортируем
    #[arg(long)]
    pub project: String,
    /// Папка со старыми задачами
    #[arg(long)]
    pub path: PathBuf,
    #[arg(long, default_value = "pithos-pandoras.db")]
    pub db: PathBuf,
}

#[derive(Debug, Deserialize)]
struct OldState {
    #[serde(rename = "задача", default)]
    number: String,
    #[serde(rename = "название", default)]
    title: String,
    #[serde(rename = "маршрут", default)]
    route: Option<String>,
    #[serde(rename = "состояние", default)]
    state: Option<String>,
    #[serde(rename = "создано", default)]
    created_at: Option<String>,
    #[serde(rename = "обновлено", default)]
    updated_at: Option<String>,
}

struct Artifact {
    path: String,
    title: String,
    kind: String,
    content: String,
    is_text: bool,
    size: Option<i64>,
}

pub async fn run(store: &Store, project: &str, path: &Path) -> anyhow::Result<()> {
    let view = store
        .read({
            let project = project.to_string();
            move |conn| projects::project_view(conn, &project)
        })
        .await
        .context("проект")?;
    let project_id = view.project.id;

    let mut imported = 0_u32;
    let mut skipped = 0_u32;

    for entry in std::fs::read_dir(path).with_context(|| format!("прочитать {}", path.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let folder = entry.path();
        let folder_name = folder
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        if folder_name == "_Шаблон" || folder_name.starts_with('.') {
            continue;
        }

        let state_path = folder.join("Состояние.json");
        if !state_path.exists() {
            continue;
        }
        let state_text = std::fs::read_to_string(&state_path)
            .with_context(|| format!("прочитать {}", state_path.display()))?;
        let old: OldState = serde_json::from_str(&state_text)
            .with_context(|| format!("разобрать {}", state_path.display()))?;

        let number: i64 = match old.number.trim().parse() {
            Ok(number) if number > 0 => number,
            _ => {
                skipped += 1;
                continue;
            }
        };

        let title = if old.title.trim().is_empty() {
            folder_name.clone()
        } else {
            old.title.clone()
        };
        let statement_path = folder.join("Задача.md");
        let statement = if statement_path.exists() {
            extract_statement(&std::fs::read_to_string(&statement_path)?)
        } else {
            String::new()
        };
        let (status_id, status_kind) = map_status(old.state.as_deref());
        let route = old.route.clone();
        let created_at = old
            .created_at
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(db::now_rfc3339);
        let updated_at = old
            .updated_at
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(db::now_rfc3339);
        let artifacts = collect_artifacts(&folder.join("Артефакты"))?;

        let result = store
            .write({
                let project_id = project_id.clone();
                let folder_name = folder_name.clone();
                let title = title.clone();
                let statement = statement.clone();
                let status_id = status_id.to_string();
                let status_kind = status_kind.to_string();
                let route = route.clone();
                let created_at = created_at.clone();
                let updated_at = updated_at.clone();
                move |conn| {
                    if tasks::get_task(conn, &project_id, &number.to_string()).is_ok() {
                        return Ok(None);
                    }

                    let tx = conn.transaction()?;
                    let status = tasks::status_by_id(&tx, &project_id, &status_id)
                        .or_else(|_| tasks::status_by_kind(&tx, &project_id, &status_kind))?;
                    let task = tasks::insert_task(
                        &tx,
                        &project_id,
                        number,
                        &folder_name,
                        &title,
                        &statement,
                        &status.id,
                        route.as_deref(),
                        None,
                        "import",
                    )?;
                    tasks::set_timestamps(&tx, &task.id, &created_at, &updated_at)?;
                    tx.execute(
                        "UPDATE project_counters
                         SET next_number = MAX(next_number, ?2)
                         WHERE project_id = ?1",
                        rusqlite::params![project_id, number + 1],
                    )?;
                    events::insert_event(
                        &tx,
                        &task.id,
                        "task_imported",
                        None,
                        Some("импорт"),
                        "{}",
                        "import",
                    )?;

                    for artifact in &artifacts {
                        if artifact.is_text {
                            reports::insert_report(
                                &tx,
                                &task.id,
                                None,
                                Some("импорт"),
                                "report",
                                &artifact.title,
                                &artifact.content,
                                &created_at,
                            )?;
                        } else {
                            files::insert_file(
                                &tx,
                                &task.id,
                                None,
                                None,
                                "импорт",
                                &artifact.kind,
                                &artifact.title,
                                &artifact.path,
                                None,
                                artifact.size,
                                &created_at,
                            )?;
                        }
                    }

                    tx.commit()?;
                    Ok(Some(task.number))
                }
            })
            .await?;

        match result {
            Some(number) => {
                imported += 1;
                println!("импортирована задача {number}: {title}");
            }
            None => {
                skipped += 1;
                println!("пропущена задача {number}: уже есть");
            }
        }
    }

    println!("импорт завершён: {imported} импортировано, {skipped} пропущено");
    Ok(())
}

fn map_status(state: Option<&str>) -> (&'static str, &'static str) {
    match state.unwrap_or("") {
        "закрыта" | "закрыто" => ("done", "done"),
        "остановлена" | "остановлено" => ("blocked", "blocked"),
        "ожидание" | "ожидает" => ("waiting", "waiting"),
        "в работе" => ("active", "active"),
        _ => ("new", "active"),
    }
}

fn extract_statement(markdown: &str) -> String {
    let mut out = String::new();
    let mut in_section = false;
    for line in markdown.lines() {
        if line.starts_with("## ") {
            if in_section {
                break;
            }
            if line.to_lowercase().contains("что сделать") {
                in_section = true;
                continue;
            }
        }
        if in_section {
            out.push_str(line);
            out.push('\n');
        }
    }
    let out = out.trim();
    if out.is_empty() {
        markdown.trim().to_string()
    } else {
        out.to_string()
    }
}

fn collect_artifacts(dir: &Path) -> std::io::Result<Vec<Artifact>> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    let mut out = Vec::new();
    for path in files {
        let title = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("файл")
            .to_string();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_text = matches!(extension.as_str(), "md" | "txt");
        let content = if is_text {
            std::fs::read_to_string(&path)?
        } else {
            String::new()
        };
        let size = std::fs::metadata(&path).ok().map(|meta| meta.len() as i64);
        out.push(Artifact {
            path: path.to_string_lossy().to_string(),
            title,
            kind: artifact_kind(&extension).to_string(),
            content,
            is_text,
            size,
        });
    }
    Ok(out)
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn artifact_kind(extension: &str) -> &'static str {
    match extension {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => "image",
        "rs" | "py" | "js" | "ts" | "bsl" | "os" | "json" | "toml" | "sql" | "html" | "css" => {
            "code"
        }
        _ => "file",
    }
}
