//! CLI приложения: serve, init, doctor, import.

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
    /// Создать пространство, статусы и роли
    Init(InitArgs),
    /// Проверить базу, схему и открытые этапы
    Doctor(DoctorArgs),
    /// Импортировать старые задачи из папки
    Import(import::ImportArgs),
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
    pub workspace: String,
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
            workspace: String::from("PithosPandoras"),
            key: None,
            key_file: PathBuf::from("pithos-pandoras.key"),
        }
    }
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub workspace: String,
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

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli
        .command
        .unwrap_or_else(|| Command::Serve(ServeArgs::default()))
    {
        Command::Serve(args) => serve(args).await,
        Command::Init(args) => init(args).await,
        Command::Doctor(args) => doctor(args).await,
        Command::Import(args) => import_old(args).await,
    }
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let key = load_or_create_key(args.key.as_deref(), &args.key_file)?;
    let store = Arc::new(Store::open(&args.db).context("открыть базу")?);
    let service = Arc::new(Service::new(store));

    if let Err(err) = service.workspace_get(Some(args.workspace.clone())).await {
        if err.code == crate::domain::ErrorCode::WorkspaceUnknown {
            eprintln!("Пространство `{}` не найдено.", args.workspace);
            eprintln!("Создайте его:");
            eprintln!(
                "  cargo run -- init --workspace {} --root . --db {}",
                args.workspace,
                args.db.display()
            );
            std::process::exit(1);
        }
        return Err(err.into());
    }

    let app = mcp::server::router(service.clone(), key)
        .merge(web::pages::router(service, args.workspace.clone()));

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

pub(crate) fn load_or_create_key(key: Option<&str>, key_file: &Path) -> anyhow::Result<String> {
    if let Some(key) = key {
        let key = key.trim();
        if key.is_empty() {
            return Err(anyhow::anyhow!("--key не может быть пустым"));
        }
        return Ok(key.to_string());
    }

    if key_file.exists() {
        let key = std::fs::read_to_string(key_file)
            .with_context(|| format!("прочитать ключ {}", key_file.display()))?;
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
    }

    let key = uuid::Uuid::now_v7().simple().to_string();
    std::fs::write(key_file, &key)
        .with_context(|| format!("записать ключ {}", key_file.display()))?;
    println!("MCP key: {key}");
    Ok(key)
}

async fn import_old(args: import::ImportArgs) -> anyhow::Result<()> {
    let store = Arc::new(Store::open(&args.db).context("открыть базу")?);
    import::run(&store, &args.workspace, &args.path).await
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
    let workspace_name = args.workspace.clone();
    let tasks_dir = args.tasks_dir.clone();
    let folder_pattern = args.folder_pattern.clone();
    let roles = args.roles.clone();
    let config = args.config.clone();

    let workspace = store
        .write(move |conn| {
            store::workspaces::init_workspace(
                conn,
                &workspace_name,
                &root_string,
                &tasks_dir,
                &folder_pattern,
                &roles,
                &config,
            )
        })
        .await
        .context("создать пространство")?;

    println!("{}", serde_json::to_string_pretty(&workspace)?);
    Ok(())
}

async fn doctor(args: DoctorArgs) -> anyhow::Result<()> {
    let store = Store::open(&args.db).context("открыть базу")?;

    let version = store
        .read(store::migrations::schema_version)
        .await
        .context("версия схемы")?;
    let names = store
        .read(store::workspaces::list_workspace_names)
        .await
        .context("пространства")?;
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
    println!("workspaces: {}", names.join(", "));
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
