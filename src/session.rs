//! Session day-buckets (ADR-0009, milestone 0005 Stage 1).
//! Persists Facade history + tool_results + surfaces per YYYY-MM-DD
//! so `kill` -> `boot` restores yesterday's conversation and canvas.
//! File: `config_dir/sessions/{date}.json`, written via `write_file_synced`
//! + `sync_dir` like `FileActionStore` (action-state-machine §5.3).

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub id: String, // YYYY-MM-DD
    pub history: Vec<String>,
    pub tool_results: Vec<StoredToolResult>,
    pub surfaces: Vec<StoredSurface>,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredToolResult {
    pub tool: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredSurface {
    pub id: String,
    pub html: String,
}

impl From<&crate::tools::ToolResult> for StoredToolResult {
    fn from(r: &crate::tools::ToolResult) -> Self {
        Self {
            tool: r.tool.to_string(),
            text: r.text.clone(),
        }
    }
}
impl From<StoredToolResult> for crate::tools::ToolResult {
    fn from(s: StoredToolResult) -> Self {
        Self {
            tool: Box::leak(s.tool.into_boxed_str()),
            text: s.text,
        }
    }
}

pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            dir: config_dir.join("sessions"),
        }
    }
    pub fn path_for(&self, date: &str) -> PathBuf {
        self.dir.join(format!("{date}.json"))
    }
    pub fn today() -> String {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self::date_from_days((secs / 86400) as i64)
    }
    /// Convert days since the Unix epoch to a `YYYY-MM-DD` UTC calendar date
    /// (Howard Hinnant's civil-from-days algorithm). Pure integer math — no
    /// libc, no external `date` binary — so day-bucket naming is identical
    /// on Linux and Windows (ADR-0011 W1).
    pub fn date_from_days(days: i64) -> String {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("{y:04}-{m:02}-{d:02}")
    }
    pub fn load(&self, date: &str) -> Option<SessionSnapshot> {
        let path = self.path_for(date);
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }
    pub fn save(&self, snap: &SessionSnapshot) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path_for(&snap.id);
        let tmp = self.dir.join(format!(".{}.tmp", snap.id));
        let bytes = serde_json::to_vec_pretty(snap).unwrap();
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &path)?;
        // fsync dir
        if let Ok(dir) = std::fs::File::open(&self.dir) {
            let _ = dir.sync_all();
        }
        Ok(())
    }
    pub fn load_or_default(&self, date: &str) -> SessionSnapshot {
        self.load(date).unwrap_or_else(|| SessionSnapshot {
            id: date.to_string(),
            history: Vec::new(),
            tool_results: Vec::new(),
            surfaces: Vec::new(),
            updated_at: crate::protocol::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn date_math_matches_known_utc_dates() {
        assert_eq!(SessionStore::date_from_days(0), "1970-01-01");
        // 54 years incl. 13 leap days -> 2024-01-01.
        assert_eq!(SessionStore::date_from_days(19_723), "2024-01-01");
        // Leap 2024 (366) + 2025 (365) + 236 days into 2026 -> 2026-08-25.
        assert_eq!(SessionStore::date_from_days(20_690), "2026-08-25");
    }
    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let snap = SessionSnapshot {
            id: "2026-08-22".into(),
            history: vec!["user: hi".into()],
            tool_results: vec![StoredToolResult {
                tool: "files.write_file".into(),
                text: "committed=true".into(),
            }],
            surfaces: vec![StoredSurface {
                id: "surface-1".into(),
                html: "<div>hi</div>".into(),
            }],
            updated_at: 1,
        };
        store.save(&snap).unwrap();
        let loaded = store.load("2026-08-22").unwrap();
        assert_eq!(loaded.history[0], "user: hi");
        assert_eq!(loaded.surfaces[0].id, "surface-1");
    }
}
