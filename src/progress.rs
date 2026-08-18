//! Real-time progress reporting for the sidebar live system graph.

use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphPhase {
    Idle,
    Planning,
    Verifying,
    Gathering,
    Composing,
    PolicyCheck,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphActivity {
    pub phase: GraphPhase,
    pub active_node_ids: Vec<String>,
    pub timestamp_ms: u64,
}

pub trait ProgressReporter: Send + Sync {
    fn report(&self, activity: GraphActivity);
}

pub type ProgressSink = Arc<dyn ProgressReporter + Send + Sync>;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
