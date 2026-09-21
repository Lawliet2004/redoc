// SPDX-License-Identifier: MIT OR Apache-2.0
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
const LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
const MAX_ROTATED_LOGS: usize = 4;

struct SizeRollingWriter {
    directory: PathBuf,
    file_name: String,
    file: Option<File>,
    bytes_written: u64,
    max_bytes: u64,
}

impl SizeRollingWriter {
    fn new(directory: &Path, file_name: &str, max_bytes: u64) -> io::Result<Self> {
        let path = directory.join(file_name);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes_written = file.metadata()?.len();
        Ok(Self {
            directory: directory.to_path_buf(),
            file_name: file_name.to_string(),
            file: Some(file),
            bytes_written,
            max_bytes: max_bytes.max(1),
        })
    }

    fn current_path(&self) -> PathBuf {
        self.directory.join(&self.file_name)
    }

    fn rotated_path(&self, index: usize) -> PathBuf {
        self.directory.join(format!("{}.{}", self.file_name, index))
    }

    fn reopen(&self) -> io::Result<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.current_path())
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            let _ = file.flush();
        }
        let result = (|| {
            let oldest = self.rotated_path(MAX_ROTATED_LOGS);
            let _ = fs::remove_file(&oldest);
            for index in (1..=MAX_ROTATED_LOGS).rev() {
                let source = if index == 1 {
                    self.current_path()
                } else {
                    self.rotated_path(index - 1)
                };
                let target = self.rotated_path(index);
                if source.exists() {
                    let _ = fs::remove_file(&target);
                    fs::rename(source, target)?;
                }
            }
            Ok(())
        })();
        self.file = Some(self.reopen()?);
        self.bytes_written = 0;
        result
    }
}

impl Write for SizeRollingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        // Keep each tracing record intact. A single unusually large record
        // may exceed the bound, but normal records never straddle rotations.
        if self.bytes_written > 0
            && self.bytes_written.saturating_add(buffer.len() as u64) > self.max_bytes
        {
            // A failed rename should not drop the log line. Reopen the
            // current file and continue; the next write can retry.
            let _ = self.rotate();
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file is unavailable"))?;
        file.write_all(buffer)?;
        self.bytes_written = self.bytes_written.saturating_add(buffer.len() as u64);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

/// Local-first audit event: who did what to which document, and when.
/// Logged at info level on the `redoc::audit` target so the existing
/// size-rolling writer captures it alongside operational logs.
pub fn audit_event(actor: &str, action: &str, document_id: &str, detail: Option<&str>) {
    let actor = crate::settings::sanitize_author_name(actor);
    match detail {
        Some(detail) => tracing::info!(
            target: "redoc::audit",
            actor = %actor,
            action = %action,
            document_id = %document_id,
            detail = %detail,
            "audit"
        ),
        None => tracing::info!(
            target: "redoc::audit",
            actor = %actor,
            action = %action,
            document_id = %document_id,
            "audit"
        ),
    }
}

pub fn init_logging(log_dir: &Path) {
    if std::fs::create_dir_all(log_dir).is_ok() {
        let Ok(file_appender) = SizeRollingWriter::new(log_dir, "redoc.log", LOG_MAX_BYTES) else {
            return;
        };
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

#[cfg(test)]
mod tests {
    use super::SizeRollingWriter;
    use std::fs;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_directory() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        // ponytail: nanos alone collide on Windows' ~15ms clock; pid disambiguates parallel tests.
        let path = std::env::temp_dir().join(format!("redoc-logging-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("create logging test directory");
        path
    }

    #[test]
    fn rolls_log_before_a_write_would_exceed_the_size_bound() {
        let directory = test_directory();
        let current = directory.join("redoc.log");
        let rotated = directory.join("redoc.log.1");
        let mut writer = SizeRollingWriter::new(&directory, "redoc.log", 8).expect("writer");
        writer.write_all(b"123456").expect("first write");
        writer.write_all(b"789").expect("second write");
        writer.flush().expect("flush");

        assert_eq!(fs::read(&rotated).expect("rotated log"), b"123456");
        assert_eq!(fs::read(&current).expect("current log"), b"789");
        fs::remove_dir_all(directory).expect("remove logging test directory");
    }

    #[test]
    fn keeps_a_bounded_number_of_rotated_logs() {
        let directory = test_directory();
        let mut writer = SizeRollingWriter::new(&directory, "redoc.log", 1).expect("writer");
        for value in b"abcdef" {
            writer
                .write_all(std::slice::from_ref(value))
                .expect("write");
        }
        writer.flush().expect("flush");

        assert!(directory.join("redoc.log").exists());
        assert!(directory.join("redoc.log.1").exists());
        assert!(directory.join("redoc.log.2").exists());
        assert!(directory.join("redoc.log.3").exists());
        assert!(directory.join("redoc.log.4").exists());
        assert!(!directory.join("redoc.log.5").exists());
        fs::remove_dir_all(directory).expect("remove logging test directory");
    }
}
