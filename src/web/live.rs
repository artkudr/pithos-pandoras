//! Поток изменений: счётчик правок базы для живой перерисовки страницы.
//!
//! Один watcher на процесс следит за версией базы через `PRAGMA data_version`.
//! Значение меняется при коммите любого другого соединения, поэтому видны и
//! действия человека, и действия агентов через MCP.
//!
//! Страница спрашивает счётчик коротким запросом. Долгое соединение (SSE)
//! не используем: браузер держит на хост не больше шести соединений, и открытая
//! страница с потоком занимала одно из них — после нескольких переходов
//! подключение вставало.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rusqlite::Connection;

/// Опрос базы раз в секунду: чаще незачем, страница всё равно перерисовывается.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct Live {
    revision: Arc<AtomicU64>,
}

impl Live {
    /// Запустить watcher. Соединение открывается сразу, чтобы отсчёт изменений
    /// начался до первых запросов страницы.
    pub fn spawn(path: PathBuf) -> Self {
        Self::spawn_every(path, POLL_INTERVAL)
    }

    pub fn spawn_every(path: PathBuf, interval: Duration) -> Self {
        let revision = Arc::new(AtomicU64::new(0));
        let counter = revision.clone();

        let conn = match crate::store::db::open_connection(&path) {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!(%err, "поток изменений: не открыть базу");
                return Self { revision };
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
                counter.fetch_add(1, Ordering::Relaxed);
            }
        });

        Self { revision }
    }

    /// Номер последнего изменения. Растёт на 1 при каждой чужой записи в базу.
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
}

fn data_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA data_version", [], |row| row.get(0))
        .unwrap_or(0)
}
