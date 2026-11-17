//! Приложение в трее Windows: сервер, страница и меню.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::cli::load_or_create_key;
use crate::mcp;
use crate::service::Service;
use crate::store::Store;
use crate::web;

enum UserEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
}

struct TrayApp {
    url: String,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    tray: Option<TrayIcon>,
    window: Option<Window>,
}

impl ApplicationHandler<UserEvent> for TrayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attributes = Window::default_attributes().with_visible(false);
            self.window = event_loop.create_window(attributes).ok();
        }

        if self.tray.is_none() {
            let scale = event_loop
                .primary_monitor()
                .map(|monitor| monitor.scale_factor())
                .unwrap_or(1.0);
            match build_tray(scale) {
                Ok(tray) => self.tray = Some(tray),
                Err(err) => {
                    tracing::error!(%err, "не удалось создать иконку в трее");
                    event_loop.exit();
                }
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(event) if event.id == "open" => open_page(&self.url),
            UserEvent::Menu(event) if event.id == "quit" => self.stop(event_loop),
            UserEvent::Tray(TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }) => open_page(&self.url),
            _ => {}
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

impl TrayApp {
    fn stop(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        event_loop.exit();
    }
}

fn open_page(url: &str) {
    if let Err(err) = open::that(url) {
        tracing::error!(%err, "не удалось открыть страницу");
    }
}

fn build_tray(scale: f64) -> anyhow::Result<TrayIcon> {
    let menu = Menu::new();
    let open = MenuItem::with_id("open", "Открыть страницу", true, None);
    let quit = MenuItem::with_id("quit", "Выход", true, None);
    menu.append_items(&[&open, &quit])?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_menu_on_right_click(true)
        .with_tooltip("PithosPandoras")
        .with_icon(tray_icon_image(scale)?)
        .build()?;
    Ok(tray)
}

/// Значок трея под масштаб экрана: 16, 24 или 32 точки.
///
/// Файлы собирает `tools/icon.ps1` из `assets/icon.svg`.
fn tray_icon_image(scale: f64) -> anyhow::Result<Icon> {
    let pixels = (16.0 * scale).round() as u32;
    let (size, rgba): (u32, &'static [u8]) = if pixels >= 28 {
        (32, include_bytes!("../assets/tray-32.rgba"))
    } else if pixels >= 20 {
        (24, include_bytes!("../assets/tray-24.rgba"))
    } else {
        (16, include_bytes!("../assets/tray-16.rgba"))
    };
    Icon::from_rgba(rgba.to_vec(), size, size).map_err(|err| anyhow::anyhow!("иконка: {err}"))
}

pub fn run() -> anyhow::Result<()> {
    init_logging();

    let db = std::env::var("PITHOS_DB").unwrap_or_else(|_| String::from("pithos-pandoras.db"));
    let project =
        std::env::var("PITHOS_PROJECT").unwrap_or_else(|_| String::from("PithosPandoras"));
    let key_file =
        std::env::var("PITHOS_KEY_FILE").unwrap_or_else(|_| String::from("pithos-pandoras.key"));
    let port: u16 = std::env::var("PITHOS_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8787);
    let key = load_or_create_key(None, Path::new(&key_file))?;
    let url = format!("http://127.0.0.1:{port}/");

    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server = std::thread::Builder::new()
        .name("pithos-server".to_string())
        .spawn(move || -> anyhow::Result<()> {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async move {
                let store = Arc::new(Store::open(Path::new(&db)).context("открыть базу")?);
                let service = Arc::new(Service::new(store));
                if let Err(err) = service.project_get(Some(project.clone())).await {
                    tracing::error!(%err, "проект не найден");
                    return Err(err.into());
                }
                let app = mcp::server::router(service.clone(), key).merge(web::pages::router(
                    service,
                    project,
                    web::live::Live::spawn(PathBuf::from(&db)),
                ));
                let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
                let _ = ready_tx.send(());
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.await;
                    })
                    .await?;
                Ok(())
            })
        })?;

    if ready_rx.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = shutdown_tx.send(());
        let _ = server.join();
        return Err(anyhow::anyhow!(
            "сервер не запустился за 10 секунд; смотрите pithos-pandoras.log"
        ));
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Tray(event));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));

    let mut app = TrayApp {
        url,
        shutdown: Some(shutdown_tx),
        tray: None,
        window: None,
    };
    let result = event_loop.run_app(&mut app);
    if let Some(tx) = app.shutdown.take() {
        let _ = tx.send(());
    }
    let server_result = server
        .join()
        .map_err(|_| anyhow::anyhow!("поток сервера упал"))?;
    result?;
    server_result?;
    Ok(())
}

fn init_logging() {
    let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let appender = tracing_appender::rolling::never(dir, "pithos-pandoras.log");
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_writer(appender)
        .with_env_filter(filter)
        .try_init();
}
