//! Слой хранения: SQLite, миграции, один writer и параллельное чтение.

pub mod comments;
pub mod db;
pub mod error;
pub mod events;
pub mod files;
pub mod migrations;
pub mod projects;
pub mod read;
pub mod reports;
pub mod stages;
pub mod tasks;
pub mod writer;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

pub use error::StoreError;
use read::Reader;
use writer::Writer;

#[derive(Clone)]
pub struct Store {
    writer: Writer,
    reader: Reader,
    path: PathBuf,
}

impl Store {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut conn = db::open_connection(&path)?;
        migrations::migrate(&mut conn)?;
        drop(conn);

        let writer_conn = db::open_connection(&path)?;
        let writer = Writer::new(writer_conn)?;
        let reader = Reader::new(path.clone());

        Ok(Self {
            writer,
            reader,
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn write<T, F>(&self, f: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        self.writer.write(f).await
    }

    pub async fn read<T, F>(&self, f: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, StoreError> + Send + 'static,
    {
        self.reader.read(f).await
    }
}
