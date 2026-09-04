//! Session day-buckets (ADR-0009, milestone 0005 Stage 1).
//! Persists Facade history + tool_results + surfaces per YYYY-MM-DD
//! so `kill` -> `boot` restores yesterday's conversation and canvas.
//! File: `config_dir/sessions/{date}.json`, written via `write_file_synced`
//! + `sync_dir` like `FileActionStore` (action-state-machine §5.3).

use serde::{Deserialize, Serialize};
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
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub layout: crate::surface::SurfaceLayout,
    #[serde(default)]
    pub bindings: Vec<String>,
    #[serde(default)]
    pub binding_values: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub stale_bindings: Vec<String>,
    #[serde(default)]
    pub data_revision: u64,
}

impl From<&crate::surface::SurfaceRecord> for StoredSurface {
    fn from(surface: &crate::surface::SurfaceRecord) -> Self {
        Self {
            id: surface.id.clone(),
            html: surface.html.clone(),
            revision: surface.revision,
            intent: surface.intent.clone(),
            layout: surface.layout.clone(),
            bindings: surface.bindings.clone(),
            binding_values: surface.binding_values.clone(),
            stale_bindings: surface.stale_bindings.clone(),
            data_revision: surface.data_revision,
        }
    }
}

impl From<StoredSurface> for crate::surface::SurfaceRecord {
    fn from(surface: StoredSurface) -> Self {
        let mut record = crate::surface::SurfaceRecord::new(
            surface.id,
            surface.intent,
            surface.html,
            surface.layout,
        );
        record.revision = surface.revision.max(1);
        if !surface.bindings.is_empty() {
            record.bindings = surface.bindings;
        }
        record.binding_values = surface.binding_values;
        record.stale_bindings = surface.stale_bindings;
        record.data_revision = surface.data_revision;
        record
    }
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
        // YYYY-MM-DD in UTC — simple, deterministic
        let days = secs / 86400;
        // Use chrono-like calc via time crate? keep simple: use `date` command fallback
        // For now use `chrono` if available, else fallback to UTC days -> string via `time`
        // To avoid dep, shell out to `date -u +%F` if secs is 0? Simpler: use `time` crate already via `crate::protocol::now`?
        // We'll just use `format` of days since epoch mod, but that's not calendar. Use `chrono` via `time` crate if present.
        // Fallback: use `std::process::Command` date
        if let Ok(out) = std::process::Command::new("date")
            .arg("-u")
            .arg("+%F")
            .output()
        {
            if let Ok(s) = String::from_utf8(out.stdout) {
                return s.trim().to_string();
            }
        }
        format!("day-{days}")
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
            surfaces: vec![StoredSurface::from(&crate::surface::SurfaceRecord::new(
                "surface-1".into(),
                "hello".into(),
                "<div>hi</div>".into(),
                crate::surface::SurfaceLayout::default(),
            ))],
            updated_at: 1,
        };
        store.save(&snap).unwrap();
        let loaded = store.load("2026-08-22").unwrap();
        assert_eq!(loaded.history[0], "user: hi");
        assert_eq!(loaded.surfaces[0].id, "surface-1");
    }
}
