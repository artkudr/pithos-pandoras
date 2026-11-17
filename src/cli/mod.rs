//! CLI приложения: serve, init, doctor, key, import, backup.

pub mod backup;
mod import;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};

use crate::mcp;
use crate::service::Service;
use crate::store::{self, Store};
use crate::web;

#[derive(Debug, Parser)]
#[command(name = "pithos-pandoras", version, about = "Локальный сервис задач")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Запустить сервер для агентов
    Serve(ServeArgs),
    /// Создать проект, статусы и роли
    Init(InitArgs),
    /// Проверить базу, схему и открытые этапы
    Doctor(DoctorArgs),
    /// Показать ключ доступа MCP
    Key(KeyArgs),
    /// Импортировать старые задачи из папки
    Import(import::ImportArgs),
    /// Сделать копию базы
    Backup(backup::BackupArgs),
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value_t = 8787)]
    pub port: u16,
    #[arg(long, default_value = "pithos-pandoras.db")]
    pub db: PathBuf,
    #[arg(long, default_value = "PithosPandoras")]
    pub project: String,
    #[arg(long)]
    pub key: Option<String>,
    #[arg(long, default_value = "pithos-pandoras.key")]
    pub key_file: PathBuf,
}

impl Default for ServeArgs {
    fn default() -> Self {
        Self {
            host: String::from("127.0.0.1"),
            port: 8787,
            db: PathBuf::from("pithos-pandoras.db"),
            project: String::from("PithosPandoras"),
            key: None,
            key_file: PathBuf::from("pithos-pandoras.key"),
        }
    }
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub project: String,
    #[arg(long, default_value = ".")]
    pub root: PathBuf,
    #[arg(long, default_value = "Задачи")]
    pub tasks_dir: String,
    #[arg(long, default_value = "{number} - {date}")]
    pub folder_pattern: String,
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "Коммандер,Исполнитель,Проверяющий"
    )]
    pub roles: Vec<String>,
    #[arg(long, default_value = "pithos-pandoras.db")]
    pub db: PathBuf,
    #[arg(long, default_value = "{}")]
    pub config: String,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long, default_value = "pithos-pandoras.db")]
    pub db: PathBuf,
}

#[derive(Debug, Args)]
pub struct KeyArgs {
    #[arg(long, default_value = "pithos-pandoras.key")]
    pub key_file: PathBuf,
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli
        .command
        .unwrap_or_else(|| Command::Serve(ServeArgs::default()))
    {
        Command::Serve(args) => serve(args).await,
        Command::Init(args) => init(args).await,
        Command::Doctor(args) => doctor(args).await,
        Command::Key(args) => key(args),
        Command::Import(args) => import_old(args).await,
        Command::Backup(args) => backup::run(args).await,
    }
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let key = load_or_create_key(args.key.as_deref(), &args.key_file)?;
    let store = Arc::new(Store::open(&args.db).context("открыть базу")?);
    let service = Arc::new(Service::new(store));

    if let Err(err) = service.project_get(Some(args.project.clone())).await {
        if err.code == crate::domain::ErrorCode::ProjectUnknown {
            eprintln!("Проект `{}` не найден.", args.project);
            eprintln!("Создайте его:");
            eprintln!(
                "  cargo run -- init --project {} --root . --db {}",
                args.project,
                args.db.display()
            );
            std::process::exit(1);
        }
        return Err(err.into());
    }

    let app = mcp::server::router(service.clone(), key).merge(web::pages::router(
        service,
        args.project.clone(),
        web::live::Live::spawn(args.db.clone()),
    ));

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("занять адрес {addr}"))?;

    println!("PithosPandoras: http://{addr}/");
    println!("MCP: http://{addr}/mcp");
    println!("MCP key file: {}", args.key_file.display());
    axum::serve(listener, app)
        .await
        .context("запустить сервер")?;
    Ok(())
}

/// Ключ из файла. `None`, если файла нет или он пуст.
pub(crate) fn read_key(key_file: &Path) -> anyhow::Result<Option<String>> {
    if !key_file.exists() {
        return Ok(None);
    }
    let key = std::fs::read_to_string(key_file)
        .with_context(|| format!("прочитать ключ {}", key_file.display()))?;
    let key = key.trim().to_string();
    Ok(if key.is_empty() { None } else { Some(key) })
}

/// Новый ключ и запись его в файл.
pub(crate) fn create_key(key_file: &Path) -> anyhow::Result<String> {
    let key = uuid::Uuid::now_v7().simple().to_string();
    std::fs::write(key_file, &key)
        .with_context(|| format!("записать ключ {}", key_file.display()))?;
    Ok(key)
}

pub(crate) fn load_or_create_key(key: Option<&str>, key_file: &Path) -> anyhow::Result<String> {
    if let Some(key) = key {
        let key = key.trim();
        if key.is_empty() {
            return Err(anyhow::anyhow!("--key не может быть пустым"));
        }
        return Ok(key.to_string());
    }

    if let Some(key) = read_key(key_file)? {
        return Ok(key);
    }

    let key = create_key(key_file)?;
    println!("MCP key: {key}");
    Ok(key)
}

/// Печатает ключ, чтобы его можно было скопировать. В stderr — откуда он взят.
fn key(args: KeyArgs) -> anyhow::Result<()> {
    let (key, created) = match read_key(&args.key_file)? {
        Some(key) => (key, false),
        None => (create_key(&args.key_file)?, true),
    };
    println!("{key}");
    if created {
        eprintln!("Ключ создан: {}", args.key_file.display());
    } else {
        eprintln!("Ключ из файла: {}", args.key_file.display());
    }
    Ok(())
}

async fn import_old(args: import::ImportArgs) -> anyhow::Result<()> {
    let store = Arc::new(Store::open(&args.db).context("открыть базу")?);
    import::run(&store, &args.project, &args.path).await
}

async fn init(args: InitArgs) -> anyhow::Result<()> {
    let store = Arc::new(Store::open(&args.db).context("открыть базу")?);
    let root = args
        .root
        .canonicalize()
        .unwrap_or_else(|_| args.root.clone());
    let root_string = root
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_string();
    let project_name = args.project.clone();
    let tasks_dir = args.tasks_dir.clone();
    let folder_pattern = args.folder_pattern.clone();
    let roles: Vec<(String, Option<String>)> =
        args.roles.iter().map(|role| (role.clone(), None)).collect();
    let config = args.config.clone();

    let project = store
        .write(move |conn| {
            store::projects::init_project(
                conn,
                &project_name,
                &root_string,
                &tasks_dir,
                &folder_pattern,
                &roles,
                &config,
            )
        })
        .await
        .context("создать проект")?;

    println!("{}", serde_json::to_string_pretty(&project)?);
    Ok(())
}

async fn doctor(args: DoctorArgs) -> anyhow::Result<()> {
    let store = Store::open(&args.db).context("открыть базу")?;

    let version = store
        .read(store::migrations::schema_version)
        .await
        .context("версия схемы")?;
    let names = store
        .read(store::projects::list_project_names)
        .await
        .context("проекты")?;
    let open_stages = store
        .read(store::stages::list_open_stages)
        .await
        .context("открытые этапы")?;
    let integrity = store
        .read(|conn| {
            let value: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
            Ok(value)
        })
        .await
        .context("integrity_check")?;
    let foreign_key_violations = store
        .read(|conn| {
            let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
            let mut rows = stmt.query([])?;
            let mut count = 0_i64;
            while rows.next()?.is_some() {
                count += 1;
            }
            Ok(count)
        })
        .await
        .context("foreign_key_check")?;
    let files = store
        .read(store::files::list_all_files)
        .await
        .context("файлы")?;
    let missing: Vec<_> = files
        .iter()
        .filter(|file| !Path::new(&file.path).exists())
        .collect();
    let port_free = std::net::TcpListener::bind(("127.0.0.1", 8787)).is_ok();

    println!("schema_version: {version}");
    println!("projects: {}", names.join(", "));
    println!("open_stages: {}", open_stages.len());
    for stage in open_stages {
        println!("  {} {} круг {}", stage.task_id, stage.role, stage.round);
    }
    println!("integrity: {integrity}");
    println!("foreign_key_violations: {foreign_key_violations}");
    println!("missing_files: {}", missing.len());
    for file in missing {
        println!("  {} {}", file.id, file.path);
    }
    println!("port_8787: {}", if port_free { "free" } else { "busy" });

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn read_key_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pithos-pandoras.key");
        assert_eq!(read_key(&file).unwrap(), None);
    }

    #[test]
    fn created_key_is_read_back_without_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pithos-pandoras.key");

        let key = create_key(&file).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), key);
        assert_eq!(read_key(&file).unwrap().as_deref(), Some(key.as_str()));
    }

    #[test]
    fn blank_key_file_counts_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pithos-pandoras.key");
        std::fs::write(&file, " \n").unwrap();
        assert_eq!(read_key(&file).unwrap(), None);
    }

    #[test]
    fn explicit_key_wins_over_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pithos-pandoras.key");
        std::fs::write(&file, "from-file").unwrap();

        let key = load_or_create_key(Some(" from-args "), &file).unwrap();
        assert_eq!(key, "from-args");
        assert_eq!(read_key(&file).unwrap().as_deref(), Some("from-file"));
    }
}
