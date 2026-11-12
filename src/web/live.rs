//! Поток изменений: SSE для живой перерисовки страницы.
//!
//! Один watcher на процесс следит за версией базы через `PRAGMA data_version`.
//! Значение меняется при коммите любого другого соединения, поэтому видны и
//! действия человека, и действия агентов через MCP.

use std::convert::Infallible;
use std::path::PathBuf;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use rusqlite::Connection;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

/// Опрос базы раз в секунду: чаще незачем, страница всё равно перерисовывается.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const CHANNEL_CAPACITY: usize = 8;

#[derive(Clone)]
pub struct Live {
    tx: broadcast::Sender<()>,
}

impl Live {
    /// Запустить watcher. Соединение открывается сразу, чтобы отсчёт изменений
    /// начался до первых запросов страницы.
    pub fn spawn(path: PathBuf) -> Self {
        Self::spawn_every(path, POLL_INTERVAL)
    }

    pub fn spawn_every(path: PathBuf, interval: Duration) -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let sender = tx.clone();

        let conn = match crate::store::db::open_connection(&path) {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!(%err, "поток изменений: не открыть базу");
                return Self { tx };
            }
        };
        let mut last = data_version(&conn);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let current = data_version(&conn);
                if current == last {
                    continue;
                }
                last = current;
                // Никто не слушает — сообщать некому, но версию уже запомнили.
                let _ = sender.send(());
            }
        });

        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }
}

/// Ответ `/events`: сообщение `change` на каждое изменение базы.
pub fn stream(live: &Live) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = live.subscribe();
    let stream = BroadcastStream::new(rx)
        .map(|_| -> Result<Event, Infallible> { Ok(Event::default().event("change").data("1")) });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn data_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA data_version", [], |row| row.get(0))
        .unwrap_or(0)
}
