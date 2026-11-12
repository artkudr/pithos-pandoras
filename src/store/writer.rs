use rusqlite::Connection;
use tokio::sync::{mpsc, oneshot};

use super::error::StoreError;

type WriteJob = Box<dyn FnOnce(&mut Connection) + Send>;

/// Один поток-писатель. Все мутации идут через него.
#[derive(Clone)]
pub struct Writer {
    tx: mpsc::UnboundedSender<WriteJob>,
}

impl Writer {
    pub fn new(conn: Connection) -> Result<Self, StoreError> {
        let (tx, mut rx) = mpsc::unbounded_channel::<WriteJob>();
        std::thread::Builder::new()
            .name("pithos-writer".to_string())
            .spawn(move || {
                let mut conn = conn;
                while let Some(job) = rx.blocking_recv() {
                    job(&mut conn);
                }
            })?;
        Ok(Self { tx })
    }

    pub async fn write<T, F>(&self, f: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        let job: WriteJob = Box::new(move |conn| {
            let result = f(conn);
            let _ = reply_tx.send(result);
        });
        self.tx
            .send(job)
            .map_err(|_| StoreError::Invalid("writer остановлен".to_string()))?;
        reply_rx
            .await
            .map_err(|_| StoreError::Invalid("writer не ответил".to_string()))?
    }
}
