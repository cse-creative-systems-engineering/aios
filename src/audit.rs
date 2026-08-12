use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

#[derive(Clone, Debug)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub outcome: String,
}

impl AuditEntry {
    fn render(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.timestamp, self.actor, self.action, self.target, self.outcome
        )
    }
}

pub struct AuditLog {
    file: Option<Mutex<File>>,
    buffer: RwLock<Vec<AuditEntry>>,
    max_buffer: usize,
}

impl AuditLog {
    pub fn new(path: Option<PathBuf>) -> Self {
        let file = if cfg!(test) {
            None
        } else {
            path.map(|path| Self::open(path))
        };
        Self {
            file: file.map(Mutex::new),
            buffer: RwLock::new(Vec::new()),
            max_buffer: 10_000,
        }
    }

    pub fn with_file(path: PathBuf) -> Self {
        Self {
            file: Some(Mutex::new(Self::open(path))),
            buffer: RwLock::new(Vec::new()),
            max_buffer: 10_000,
        }
    }

    fn open(path: PathBuf) -> File {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("open audit log")
    }

    pub fn disabled() -> Self {
        Self {
            file: None,
            buffer: RwLock::new(Vec::new()),
            max_buffer: 10_000,
        }
    }

    pub fn record(&self, actor: &str, action: &str, target: &str, outcome: &str) {
        let entry = AuditEntry {
            timestamp: crate::protocol::now(),
            actor: actor.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            outcome: outcome.to_string(),
        };
        if let Some(file) = &self.file {
            let mut file = file.lock().expect("audit file lock");
            let _ = writeln!(file, "{}", entry.render());
            let _ = file.flush();
        }
        let mut buffer = self.buffer.write().expect("audit buffer lock");
        buffer.push(entry);
        if buffer.len() > self.max_buffer {
            let excess = buffer.len() - self.max_buffer;
            buffer.drain(0..excess);
        }
    }

    pub fn entries(&self) -> Vec<AuditEntry> {
        self.buffer.read().expect("audit buffer lock").clone()
    }

    pub fn last(&self) -> Option<AuditEntry> {
        self.buffer.read().expect("audit buffer lock").last().cloned()
    }

    pub fn filter(&self, actor: &str) -> Vec<AuditEntry> {
        self.entries()
            .into_iter()
            .filter(|e| e.actor == actor)
            .collect()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(None)
    }
}

pub fn audit_log_path(config_dir: &Path) -> PathBuf {
    config_dir.join("audit.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_log() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audit.log");
        (dir, path)
    }

    #[test]
    fn records_to_buffer() {
        let log = AuditLog::disabled();
        log.record("user", "chat", "hello", "ok");
        log.record("facade", "scan", "system", "ok");
        let entries = log.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].actor, "user");
        assert_eq!(entries[0].action, "chat");
        assert_eq!(log.last().expect("last").action, "scan");
    }

    #[test]
    fn filter_by_actor() {
        let log = AuditLog::disabled();
        log.record("user", "chat", "a", "ok");
        log.record("facade", "scan", "system", "ok");
        log.record("user", "plan", "b", "ok");
        let user_entries = log.filter("user");
        assert_eq!(user_entries.len(), 2);
    }

    #[test]
    fn writes_to_file() {
        let (_dir, path) = tmp_log();
        let log = AuditLog::with_file(path.clone());
        log.record("user", "chat", "hello", "ok");
        drop(log);
        let text = std::fs::read_to_string(&path).expect("read log");
        assert!(text.contains("chat"), "{text}");
        assert!(text.contains("hello"), "{text}");
    }

    #[test]
    fn appends_across_sessions() {
        let (_dir, path) = tmp_log();
        {
            let log = AuditLog::with_file(path.clone());
            log.record("user", "chat", "first", "ok");
        }
        let log = AuditLog::with_file(path.clone());
        log.record("user", "plan", "second", "ok");
        let text = std::fs::read_to_string(&path).expect("read log");
        assert!(text.contains("first"), "{text}");
        assert!(text.contains("second"), "{text}");
    }
}
