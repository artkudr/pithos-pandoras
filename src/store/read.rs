use std::path::PathBuf;

use rusqlite::Connection;

use super::db::open_connection;
use super::error::StoreError;

/// Чтение идёт параллельно: на каждый запрос своё короткое соединение.
#[derive(Clone)]
pub struct Reader {
    path: PathBuf,
}

impl Reader {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub async fn read<T, F>(&self, f: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let conn = open_connection(&path)?;
            f(&conn)
        })
        .await
        .map_err(|err| StoreError::Invalid(format!("read task: {err}")))?
    }
}
