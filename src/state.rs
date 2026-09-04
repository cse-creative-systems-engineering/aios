//! Live, deterministic observation store (ADR-0012).
//!
//! This is intentionally separate from `SystemGraph`: the graph describes
//! topology and ownership, while this store retains timestamped facts and
//! bounded history for context projections. Neither is an authority boundary.

use crate::graph::SystemGraph;
use crate::protocol::{DataClassification, HealthState, Timestamp, now};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

pub const DEFAULT_HISTORY_LIMIT: usize = 60;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateSample {
    pub value: String,
    pub observed_at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateMetric {
    pub key: String,
    pub resource: String,
    pub value: String,
    pub unit: Option<String>,
    pub observed_at: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub source: String,
    pub classification: DataClassification,
    pub history: VecDeque<StateSample>,
}

impl StateMetric {
    pub fn is_stale(&self, timestamp: Timestamp) -> bool {
        self.expires_at.is_some_and(|expires| timestamp > expires)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateFinding {
    pub key: String,
    pub kind: String,
    pub detail: String,
    pub observed_from: Timestamp,
    pub observed_to: Timestamp,
    pub source_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextFact {
    pub key: String,
    pub value: String,
    pub unit: Option<String>,
    pub resource: String,
    pub source: String,
    pub observed_at: Timestamp,
    pub freshness: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextProjection {
    pub generated_at: Timestamp,
    pub query: String,
    pub facts: Vec<ContextFact>,
    pub findings: Vec<StateFinding>,
    pub truncated: bool,
}

impl ContextProjection {
    pub fn as_prompt_context(&self) -> String {
        let mut lines = vec!["Live system context (deterministic observations):".to_string()];
        for fact in &self.facts {
            let unit = fact.unit.as_deref().unwrap_or("");
            lines.push(format!(
                "- {} = {}{} [resource={}, source={}, observed_at={}, freshness={}]",
                fact.key, fact.value, unit, fact.resource, fact.source, fact.observed_at, fact.freshness
            ));
        }
        for finding in &self.findings {
            lines.push(format!(
                "- finding ({}, non-causal): {} [keys={}, window={}..{}]",
                finding.kind,
                finding.detail,
                finding.source_keys.join(","),
                finding.observed_from,
                finding.observed_to
            ));
        }
        if self.truncated {
            lines.push("- projection truncated; use a typed read-only query for more detail.".into());
        }
        lines.join("\n")
    }
}

#[derive(Clone, Debug)]
pub struct SystemStateStore {
    metrics: BTreeMap<String, StateMetric>,
    history_limit: usize,
}

impl Default for SystemStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemStateStore {
    pub fn new() -> Self {
        Self::with_history_limit(DEFAULT_HISTORY_LIMIT)
    }

    pub fn with_history_limit(history_limit: usize) -> Self {
        Self { metrics: BTreeMap::new(), history_limit: history_limit.max(2) }
    }

    pub fn publish(
        &mut self,
        key: impl Into<String>,
        resource: impl Into<String>,
        value: impl Into<String>,
        unit: Option<String>,
        observed_at: Timestamp,
        expires_at: Option<Timestamp>,
        source: impl Into<String>,
        classification: DataClassification,
    ) {
        let key = key.into();
        let resource = resource.into();
        let value = value.into();
        let source = source.into();
        let sample = StateSample { value: value.clone(), observed_at };
        let metric = self.metrics.entry(key.clone()).or_insert_with(|| StateMetric {
            key: key.clone(),
            resource: resource.clone(),
            value: value.clone(),
            unit: unit.clone(),
            observed_at,
            expires_at,
            source: source.clone(),
            classification,
            history: VecDeque::new(),
        });
        metric.resource = resource;
        metric.value = value;
        metric.unit = unit;
        metric.observed_at = observed_at;
        metric.expires_at = expires_at;
        metric.source = source;
        metric.classification = classification;
        if metric.history.back() != Some(&sample) {
            metric.history.push_back(sample);
            while metric.history.len() > self.history_limit {
                metric.history.pop_front();
            }
        }
    }

    /// Seed/update observations available from deterministic discovery. This
    /// deliberately copies only public system configuration metadata.
    pub fn ingest_graph(&mut self, graph: &SystemGraph) {
        for node in graph.nodes().values() {
            let prefix = format!("{:?}.{}", node.node_type, sanitize(&node.node_id.0)).to_ascii_lowercase();
            self.publish(
                format!("{prefix}.health"),
                node.node_id.0.clone(),
                health_name(&node.health),
                None,
                node.last_observed,
                node.expires_at,
                "system_graph",
                DataClassification::SystemConfig,
            );
            for (name, value) in &node.attributes {
                self.publish(
                    format!("{prefix}.{}", sanitize(name)),
                    node.node_id.0.clone(),
                    value.clone(),
                    None,
                    node.last_observed,
                    node.expires_at,
                    "discovery",
                    DataClassification::SystemConfig,
                );
            }
        }
    }

    /// Refresh portable host observations immediately before a projection.
    /// These read-only Linux interfaces are optional: unavailable sources are
    /// absent, never reported as healthy or fabricated.
    #[cfg(target_os = "linux")]
    pub fn refresh_host(&mut self) {
        let observed_at = now();
        if let Ok(loadavg) = std::fs::read_to_string("/proc/loadavg") {
            if let Some(load1) = loadavg.split_whitespace().next() {
                self.publish("cpu.load_1m", "cpu:all", load1, None, observed_at, Some(observed_at + 15), "procfs", DataClassification::SystemConfig);
            }
        }
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for key in ["MemTotal", "MemAvailable"] {
                if let Some(value) = meminfo.lines().find_map(|line| line.strip_prefix(&format!("{key}:"))).and_then(|value| value.split_whitespace().next()) {
                    self.publish(format!("memory.{}", sanitize(key)), "memory:system", value, Some("kB".into()), observed_at, Some(observed_at + 15), "procfs", DataClassification::SystemConfig);
                }
            }
        }
        if let Ok(network) = std::fs::read_to_string("/proc/net/dev") {
            for line in network.lines().skip(2) {
                let Some((interface, counters)) = line.split_once(':') else { continue; };
                let values: Vec<&str> = counters.split_whitespace().collect();
                if values.len() < 9 { continue; }
                let interface = sanitize(interface.trim());
                self.publish(format!("network.{interface}.rx_bytes"), format!("network:{interface}"), values[0], Some("B".into()), observed_at, Some(observed_at + 15), "procfs", DataClassification::SystemConfig);
                self.publish(format!("network.{interface}.tx_bytes"), format!("network:{interface}"), values[8], Some("B".into()), observed_at, Some(observed_at + 15), "procfs", DataClassification::SystemConfig);
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn refresh_host(&mut self) {}

    pub fn metric(&self, key: &str) -> Option<&StateMetric> { self.metrics.get(key) }

    pub fn project(&self, query: &str, limit: usize) -> ContextProjection {
        let timestamp = now();
        let terms = query_terms(query);
        let mut candidates: Vec<&StateMetric> = self.metrics.values()
            .filter(|metric| metric.classification != DataClassification::Protected)
            .collect();
        candidates.sort_by_key(|metric| {
            let haystack = format!("{} {} {}", metric.key, metric.resource, metric.source).to_ascii_lowercase();
            let matches = terms.iter().filter(|term| haystack.contains(term.as_str())).count();
            (std::cmp::Reverse(matches), metric.key.clone())
        });
        if !terms.is_empty() && candidates.iter().any(|metric| {
            let haystack = format!("{} {}", metric.key, metric.resource).to_ascii_lowercase();
            terms.iter().any(|term| haystack.contains(term.as_str()))
        }) {
            candidates.retain(|metric| {
                let haystack = format!("{} {}", metric.key, metric.resource).to_ascii_lowercase();
                terms.iter().any(|term| haystack.contains(term.as_str()))
            });
        }
        let truncated = candidates.len() > limit;
        let facts = candidates.into_iter().take(limit).map(|metric| ContextFact {
            key: metric.key.clone(), value: metric.value.clone(), unit: metric.unit.clone(),
            resource: metric.resource.clone(), source: metric.source.clone(),
            observed_at: metric.observed_at,
            freshness: if metric.is_stale(timestamp) { "stale".into() } else { "fresh".into() },
        }).collect();
        let findings = self.findings(query).into_iter().take(limit.saturating_div(4).max(1)).collect();
        ContextProjection { generated_at: timestamp, query: query.into(), facts, findings, truncated }
    }

    pub fn findings(&self, query: &str) -> Vec<StateFinding> {
        let terms = query_terms(query);
        self.metrics.values().filter_map(|metric| {
            if !terms.is_empty() && !terms.iter().any(|term| metric.key.to_ascii_lowercase().contains(term)) { return None; }
            let first = metric.history.front()?;
            let last = metric.history.back()?;
            if first.observed_at == last.observed_at { return None; }
            let from = first.value.parse::<f64>().ok()?;
            let to = last.value.parse::<f64>().ok()?;
            let delta = to - from;
            if delta.abs() < f64::EPSILON { return None; }
            Some(StateFinding {
                key: metric.key.clone(),
                kind: if delta > 0.0 { "rising".into() } else { "falling".into() },
                detail: format!("{} changed by {:+.2} between samples", metric.key, delta),
                observed_from: first.observed_at, observed_to: last.observed_at,
                source_keys: vec![metric.key.clone()],
            })
        }).collect()
    }
}

fn query_terms(query: &str) -> Vec<String> {
    query.to_ascii_lowercase().split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|term| term.len() >= 3).map(str::to_string).collect()
}

fn sanitize(value: &str) -> String {
    value.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect()
}

fn health_name(health: &HealthState) -> &'static str {
    match health { HealthState::Healthy => "healthy", HealthState::Degraded => "degraded", HealthState::Unhealthy => "unhealthy", HealthState::Unknown => "unknown", HealthState::Stale => "stale" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_bounded_history_and_reports_non_causal_trend() {
        let mut state = SystemStateStore::with_history_limit(2);
        state.publish("gpu.0.temperature_c", "gpu:0", "70", Some("C".into()), 10, None, "nvml", DataClassification::SystemConfig);
        state.publish("gpu.0.temperature_c", "gpu:0", "82", Some("C".into()), 20, None, "nvml", DataClassification::SystemConfig);
        let findings = state.findings("gpu temperature");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "rising");
        assert!(findings[0].detail.contains("+12.00"));
    }

    #[test]
    fn projection_is_relevant_and_bounded() {
        let mut state = SystemStateStore::new();
        state.publish("cpu.0.utilization", "cpu:0", "54", Some("%".into()), 10, None, "procfs", DataClassification::SystemConfig);
        state.publish("network.eth0.rx_bps", "device:net-eth0", "99", None, 10, None, "procfs", DataClassification::SystemConfig);
        let projection = state.project("", 1);
        assert_eq!(projection.facts.len(), 1);
        assert!(projection.facts[0].key.contains("cpu"));
        assert!(projection.truncated);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn host_refresh_publishes_procfs_observations_when_available() {
        let mut state = SystemStateStore::new();
        state.refresh_host();
        assert!(state.metric("cpu.load_1m").is_some());
        assert!(state.metrics.keys().any(|key| key.starts_with("network.")));
    }
}
