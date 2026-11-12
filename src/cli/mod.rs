//! CLI приложения: serve, init, doctor.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};

use crate::mcp;
use crate::service::Service;
use crate::store::{self, Store};

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
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value_t = 8787)]
    pub port: u16,
    #[arg(long, default_value = "pithos-pandoras.db")]
    pub db: PathBuf,
}

impl Default for ServeArgs {
    fn default() -> Self {
        Self {
            host: String::from("127.0.0.1"),
            port: 8787,
            db: PathBuf::from("pithos-pandoras.db"),
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
    }
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let store = Arc::new(Store::open(&args.db).context("открыть базу")?);
    let service = Arc::new(Service::new(store));
    let app = mcp::server::router(service);

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("занять адрес {addr}"))?;

    println!("PithosPandoras: http://{addr}/mcp");
    axum::serve(listener, app)
        .await
        .context("запустить сервер")?;
    Ok(())
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

    println!("schema_version: {version}");
    println!("workspaces: {}", names.join(", "));
    println!("open_stages: {}", open_stages.len());
    for stage in open_stages {
        println!("  {} {} круг {}", stage.task_id, stage.role, stage.round);
    }

    Ok(())
}
