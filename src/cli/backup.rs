//! Резервная копия базы: снимок SQLite через `VACUUM INTO`.

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Args;
use rusqlite::{Connection, OpenFlags};

use crate::store::{self, Store};

#[derive(Debug, Args)]
pub struct BackupArgs {
    #[arg(long, default_value = "pithos-pandoras.db")]
    pub db: PathBuf,
    /// Каталог для копий. По умолчанию — `backups` рядом с базой.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub async fn run(args: BackupArgs) -> anyhow::Result<()> {
    let store = Store::open(&args.db).context("открыть базу")?;
    let out_dir = args.out.clone().unwrap_or_else(|| default_dir(&args.db));

    let path = backup_to(&store, &out_dir).await?;
    let size = std::fs::metadata(&path)
        .map(|meta| meta.len())
        .unwrap_or_default();

    println!("Копия: {}", path.display());
    println!("Размер: {}", human_size(size));

    let projects = store
        .read(store::projects::list_projects)
        .await
        .context("проекты")?;
    if projects.is_empty() {
        println!("Проектов нет.");
    } else {
        println!("Файлы задач в копию не входят, скопируйте папки отдельно:");
        for project in projects {
            println!("  {} — {}", project.name, project.root_path);
        }
    }
    Ok(())
}

/// Сделать снимок базы в каталог `dir` и проверить его.
pub async fn backup_to(store: &Store, dir: &Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("создать каталог {}", dir.display()))?;
    let path = unique_path(dir);
    let target = path.to_string_lossy().to_string();

    store
        .write(move |conn| {
            conn.execute("VACUUM INTO ?1", [target.as_str()])?;
            Ok(())
        })
        .await
        .context("скопировать базу")?;

    verify(&path)?;
    Ok(path)
}

fn verify(path: &Path) -> anyhow::Result<()> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("открыть копию {}", path.display()))?;
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .context("проверить копию")?;
    if integrity != "ok" {
        return Err(anyhow::anyhow!(
            "копия повреждена: {integrity}; файл {}",
            path.display()
        ));
    }
    let tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'tasks'",
            [],
            |row| row.get(0),
        )
        .context("проверить схему копии")?;
    if tables == 0 {
        return Err(anyhow::anyhow!(
            "в копии нет таблицы tasks: {}",
            path.display()
        ));
    }
    Ok(())
}

fn default_dir(db: &Path) -> PathBuf {
    match db.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join("backups"),
        _ => PathBuf::from("backups"),
    }
}

fn unique_path(dir: &Path) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let base = format!("pithos-pandoras-{stamp}");
    let mut path = dir.join(format!("{base}.db"));
    let mut index = 2;
    while path.exists() {
        path = dir.join(format!("{base}-{index}.db"));
        index += 1;
    }
    path
}

fn human_size(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    if bytes >= MB {
        format!("{:.1} МБ", bytes as f64 / MB as f64)
    } else {
        format!("{} КБ", bytes.div_ceil(1024))
    }
}
