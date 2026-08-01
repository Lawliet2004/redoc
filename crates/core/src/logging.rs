use std::path::Path;
use std::sync::OnceLock;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

pub fn init_logging(log_dir: &Path) {
    if std::fs::create_dir_all(log_dir).is_ok() {
        // tracing-appender 0.2 has daily/hourly/minutely rotation only (no size-based rolling).
        // Daily rotation approximates the 5 MB target documented in crates/README-deps.md.
        let file_appender = tracing_appender::rolling::daily(log_dir, "redoc.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        if tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .with_ansi(false)
            .try_init()
            .is_ok()
        {
            let _ = LOG_GUARD.set(guard);
        }
    }
}
