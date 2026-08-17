use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

/// A failure to write to the audit log. The audit log is in the TCB
/// (security-model.md §1.1); a write failure must fail the operation closed,
/// never be silently ignored (observability.md §1.7, REQ-OBS-001).
#[derive(Debug)]
pub enum AuditError {
    WriteFailed(String),
    ChainVerificationFailed(String),
    Malformed(String),
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::WriteFailed(reason) => write!(f, "audit write failed: {reason}"),
            AuditError::ChainVerificationFailed(reason) => {
                write!(f, "audit chain verification failed: {reason}")
            }
            AuditError::Malformed(reason) => write!(f, "malformed audit log: {reason}"),
        }
    }
}

impl std::error::Error for AuditError {}

#[derive(Clone, Debug)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub outcome: String,
    /// SHA-256 of the previous entry in the chain. `[0; 32]` for the first entry.
    /// (observability.md §1.3/§1.5, message-protocol.md §6.3)
    pub previous_entry_hash: [u8; 32],
    /// SHA-256 of `contents ++ previous_entry_hash` for this entry.
    pub entry_hash: [u8; 32],
}

impl AuditEntry {
    fn contents(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.timestamp,
            encode_field(&self.actor),
            encode_field(&self.action),
            encode_field(&self.target),
            encode_field(&self.outcome)
        )
    }

    fn render(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.timestamp,
            encode_field(&self.actor),
            encode_field(&self.action),
            encode_field(&self.target),
            encode_field(&self.outcome),
            hex(&self.previous_entry_hash),
            hex(&self.entry_hash),
        )
    }

    /// Compute the entry hash per observability.md §1.5:
    /// `entry.hash = SHA256(entry.contents + entry.previous_entry_hash)`.
    fn compute_hash(contents: &str, previous_hash: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(contents.as_bytes());
        hasher.update(previous_hash);
        hasher.finalize().into()
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn encode_field(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('|', "%7C")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

fn parse_hash(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

pub struct AuditLog {
    file: Option<Mutex<File>>,
    buffer: RwLock<Vec<AuditEntry>>,
    max_buffer: usize,
    /// Last entry hash. `[0; 32]` when empty. Chained into the next entry.
    last_hash: Mutex<[u8; 32]>,
}

impl AuditLog {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self::try_new(path).expect("audit log failed to initialize")
    }

    pub fn try_new(path: Option<PathBuf>) -> Result<Self, AuditError> {
        let last_hash = Self::load_last_hash(path.as_deref())?;
        let file = if cfg!(test) {
            None
        } else {
            path.as_deref().map(Self::open_checked).transpose()?
        };
        Ok(Self {
            file: file.map(Mutex::new),
            buffer: RwLock::new(Vec::new()),
            max_buffer: 10_000,
            last_hash: Mutex::new(last_hash),
        })
    }

    pub fn with_file(path: PathBuf) -> Self {
        let last_hash = Self::load_last_hash(Some(&path)).expect("audit log malformed");
        Self {
            file: Some(Mutex::new(Self::open(path))),
            buffer: RwLock::new(Vec::new()),
            max_buffer: 10_000,
            last_hash: Mutex::new(last_hash),
        }
    }

    /// Read the last entry's hash from an existing on-disk log so a new
    /// session continues the forward chain rather than breaking it
    /// (observability.md §1.5). Returns `[0; 32]` if the file is empty or
    /// unreadable (first entry or fail-fast on malformed).
    fn load_last_hash(path: Option<&Path>) -> Result<[u8; 32], AuditError> {
        let Some(path) = path else {
            return Ok([0u8; 32]);
        };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok([0u8; 32]);
            }
            Err(error) => return Err(AuditError::WriteFailed(error.to_string())),
        };
        let last_line = match text.lines().last() {
            Some(l) if !l.trim().is_empty() => l,
            _ => return Ok([0u8; 32]),
        };
        let fields: Vec<&str> = last_line.split('|').collect();
        match fields.len() {
            // Legacy v0.1 format (space-separated, no hash fields) predates
            // hash chaining. Those entries cannot be verified retroactively,
            // so the chain restarts from zero after an upgrade.
            1 => return Ok([0u8; 32]),
            7 => {}
            _ => return Err(AuditError::Malformed(format!(
                "last entry has {} fields; expected 7",
                fields.len()
            ))),
        }
        parse_hash(fields[6]).ok_or_else(|| {
            AuditError::Malformed("last entry hash is not 64 hexadecimal characters".into())
        })
    }

    fn open_checked(path: &Path) -> Result<File, AuditError> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| AuditError::WriteFailed(error.to_string()))
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
            last_hash: Mutex::new([0u8; 32]),
        }
    }

    /// Append an entry with forward-chained SHA-256 hashes, writing it to the
    /// audit file and the in-memory buffer. Returns an error if the file write
    /// fails so callers can fail closed (observability.md §1.7).
    pub fn record(
        &self,
        actor: &str,
        action: &str,
        target: &str,
        outcome: &str,
    ) -> Result<(), AuditError> {
        let contents = format!(
            "{} {} {} {} {}",
            crate::protocol::now(),
            encode_field(actor),
            encode_field(action),
            encode_field(target),
            encode_field(outcome)
        );
        let previous_hash = *self.last_hash.lock().expect("audit hash lock");
        let entry_hash = AuditEntry::compute_hash(&contents, &previous_hash);
        let entry = AuditEntry {
            timestamp: crate::protocol::now(),
            actor: actor.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            outcome: outcome.to_string(),
            previous_entry_hash: previous_hash,
            entry_hash,
        };

        if let Some(file) = &self.file {
            let mut file = file.lock().map_err(|e| {
                AuditError::WriteFailed(format!("audit file lock poisoned: {e}"))
            })?;
            write_checked(&mut file, &entry)?;
            file.flush().map_err(|e| {
                AuditError::WriteFailed(format!("flush failed: {e}"))
            })?;
        }

        *self.last_hash.lock().expect("audit hash lock") = entry_hash;
        let mut buffer = self.buffer.write().expect("audit buffer lock");
        buffer.push(entry);
        if buffer.len() > self.max_buffer {
            let excess = buffer.len() - self.max_buffer;
            buffer.drain(0..excess);
        }
        Ok(())
    }

    /// Verify the hash chain of every in-memory entry. Returns the entry index
    /// of the first hash mismatch, if any. `None` means the chain is intact.
    /// (security-model.md §3.2: tampering is detectable.)
    pub fn verify_chain(&self) -> Option<usize> {
        let buffer = self.buffer.read().expect("audit buffer lock");
        let mut prev = [0u8; 32];
        for (i, entry) in buffer.iter().enumerate() {
            if entry.previous_entry_hash != prev {
                return Some(i);
            }
            let expected = AuditEntry::compute_hash(&entry.contents(), &prev);
            if entry.entry_hash != expected {
                return Some(i);
            }
            prev = entry.entry_hash;
        }
        None
    }

    /// Test-only seam: corrupt an in-memory entry's outcome to simulate file
    /// tampering, so the chain verification can be exercised.
    #[cfg(test)]
    fn corrupt_entry_for_test(&self, index: usize, new_outcome: &str) {
        let mut buffer = self.buffer.write().expect("audit buffer lock");
        if let Some(entry) = buffer.get_mut(index) {
            entry.outcome = new_outcome.to_string();
        }
    }

    /// Set the initial chain hash to match an existing on-disk log so a new
    /// session continues the chain rather than breaking it.
    pub fn set_last_hash(&self, hash: [u8; 32]) {
        *self.last_hash.lock().expect("audit hash lock") = hash;
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

fn write_checked(file: &mut File, entry: &AuditEntry) -> Result<(), AuditError> {
    writeln!(file, "{}", entry.render())
        .map_err(|e| AuditError::WriteFailed(e.to_string()))
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
        log.record("user", "chat", "hello", "ok").unwrap();
        log.record("facade", "scan", "system", "ok").unwrap();
        let entries = log.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].actor, "user");
        assert_eq!(entries[0].action, "chat");
        assert_eq!(log.last().expect("last").action, "scan");
    }

    #[test]
    fn filter_by_actor() {
        let log = AuditLog::disabled();
        log.record("user", "chat", "a", "ok").unwrap();
        log.record("facade", "scan", "system", "ok").unwrap();
        log.record("user", "plan", "b", "ok").unwrap();
        let user_entries = log.filter("user");
        assert_eq!(user_entries.len(), 2);
    }

    #[test]
    fn writes_to_file() {
        let (_dir, path) = tmp_log();
        let log = AuditLog::with_file(path.clone());
        log.record("user", "chat", "hello", "ok").unwrap();
        drop(log);
        let text = std::fs::read_to_string(&path).expect("read log");
        assert!(text.contains("chat"), "{text}");
        assert!(text.contains("hello"), "{text}");
    }

    #[test]
    fn fields_with_record_delimiters_stay_on_one_line() {
        let (_dir, path) = tmp_log();
        let log = AuditLog::with_file(path.clone());
        log.record("user", "chat", "line one\nline two|extra", "ok")
            .unwrap();
        drop(log);
        let text = std::fs::read_to_string(&path).expect("read log");
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("%0A"), "{text}");
        assert!(text.contains("%7C"), "{text}");
    }

    #[test]
    fn appends_across_sessions() {
        let (_dir, path) = tmp_log();
        {
            let log = AuditLog::with_file(path.clone());
            log.record("user", "chat", "first", "ok").unwrap();
        }
        let log = AuditLog::with_file(path.clone());
        log.record("user", "plan", "second", "ok").unwrap();
        let text = std::fs::read_to_string(&path).expect("read log");
        assert!(text.contains("first"), "{text}");
        assert!(text.contains("second"), "{text}");
    }

    #[test]
    fn entries_are_forward_chained() {
        let log = AuditLog::disabled();
        log.record("user", "chat", "a", "ok").unwrap();
        log.record("facade", "scan", "system", "ok").unwrap();
        log.record("planner", "tool", "observe", "ok").unwrap();
        let entries = log.entries();
        // First entry: previous hash is zero.
        assert_eq!(entries[0].previous_entry_hash, [0u8; 32]);
        // Each entry's previous hash is the previous entry's entry hash.
        assert_eq!(entries[1].previous_entry_hash, entries[0].entry_hash);
        assert_eq!(entries[2].previous_entry_hash, entries[1].entry_hash);
        // Entry hashes are non-zero.
        assert_ne!(entries[0].entry_hash, [0u8; 32]);
        assert!(log.verify_chain().is_none());
    }

    #[test]
    fn tampering_is_detected() {
        let log = AuditLog::disabled();
        log.record("user", "chat", "a", "ok").unwrap();
        log.record("facade", "scan", "system", "ok").unwrap();
        log.record("planner", "tool", "observe", "ok").unwrap();
        assert!(log.verify_chain().is_none());
        // Corrupt the middle entry's content (simulating log tampering).
        log.corrupt_entry_for_test(1, "tampered");
        assert!(log.verify_chain().is_some());
    }

    #[test]
    fn read_only_audit_path_fails_fast() {
        let (_dir, path) = tmp_log();
        // Create the log with one entry, then make it read-only.
        {
            let log = AuditLog::with_file(path.clone());
            log.record("user", "chat", "a", "ok").unwrap();
        }
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).expect("chmod");
        // Opening an append-mode audit file that is read-only must fail fast
        // (ADR-0003) — a log that cannot be written must not be silently used.
        let result = std::panic::catch_unwind(|| {
            let _ = AuditLog::with_file(path);
        });
        assert!(result.is_err(), "read-only audit file must fail fast");
    }
}
