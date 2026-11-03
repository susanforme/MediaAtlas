//! Local infrastructure adapters: structured logs and SQLite persistence.

pub mod library;
pub mod tasks;

use std::{io, path::Path};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Keep this guard alive until process exit so buffered log records are flushed.
pub fn init_logging(directory: &Path) -> Result<WorkerGuard, Box<dyn std::error::Error>> {
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("mediaatlas")
        .filename_suffix("jsonl")
        .max_log_files(7)
        .build(directory)
        .map_err(|_| io::Error::other("Application log directory is unavailable."))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().json().with_writer(writer))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(io::stderr),
        )
        .try_init()
        .map_err(|_| io::Error::other("Application logging initialization failed."))?;
    Ok(guard)
}

/// No user paths, credentials, URLs, query strings or raw IPC payloads are accepted.
pub fn log_app_ready() {
    tracing::info!(event = "app_ready", "Desktop application initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn diagnostics_are_structured_and_contain_no_user_payload() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = Buffer(Arc::clone(&bytes));
        let subscriber = tracing_subscriber::fmt()
            .json()
            .without_time()
            .with_writer(move || output.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, log_app_ready);
        let log: serde_json::Value = serde_json::from_slice(&bytes.lock().unwrap()).unwrap();
        assert_eq!(log["level"], "INFO");
        assert_eq!(log["fields"]["event"], "app_ready");
        assert_eq!(log["fields"].as_object().unwrap().len(), 2);
    }
}
