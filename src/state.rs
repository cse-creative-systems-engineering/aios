//! Live, deterministic observation store (ADR-0012).
//!
//! This is intentionally separate from `SystemGraph`: the graph describes
//! topology and ownership, while this store retains timestamped facts and
//! bounded history for context projections. Neither is an authority boundary.

use crate::gpu_runtime::{GpuRuntimeAdapter, GpuRuntimeSamples};
use crate::graph::SystemGraph;
use crate::protocol::{DataClassification, HealthState, Timestamp, now};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};

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
    #[serde(default)]
    pub confidence_rule: String,
    #[serde(default)]
    pub freshness: String,
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

/// Exact values and explicit freshness for one surface's declared bindings.
/// A missing metric is deliberately absent from both collections: absence is
/// not a claim that a value was healthy, stale, or zero.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BindingSnapshot {
    pub values: BTreeMap<String, String>,
    pub stale_keys: Vec<String>,
}

impl ContextProjection {
    pub fn as_prompt_context(&self) -> String {
        let mut lines = vec!["Live system context (deterministic observations):".to_string()];
        for fact in &self.facts {
            let unit = fact.unit.as_deref().unwrap_or("");
            lines.push(format!(
                "- {} = {}{} [resource={}, source={}, observed_at={}, freshness={}]",
                fact.key,
                fact.value,
                unit,
                fact.resource,
                fact.source,
                fact.observed_at,
                fact.freshness
            ));
        }
        for finding in &self.findings {
            lines.push(format!(
                "- finding ({}, non-causal): {} [keys={}, window={}..{}, rule={}, freshness={}]",
                finding.kind,
                finding.detail,
                finding.source_keys.join(","),
                finding.observed_from,
                finding.observed_to,
                finding.confidence_rule,
                finding.freshness,
            ));
        }
        if self.truncated {
            lines.push(
                "- projection truncated; use a typed read-only query for more detail.".into(),
            );
        }
        lines.join("\n")
    }
}

#[derive(Clone, Debug)]
pub struct SystemStateStore {
    metrics: BTreeMap<String, StateMetric>,
    history_limit: usize,
    host_cpu_ticks: Option<(u64, u64)>,
    host_process_ticks: HashMap<u32, u64>,
    host_network_bytes: HashMap<String, (u64, u64, Timestamp)>,
    gpu_adapter: Option<GpuRuntimeAdapter>,
    gpu_adapter_checked: bool,
    last_gpu_refresh: Option<Timestamp>,
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
        Self {
            metrics: BTreeMap::new(),
            history_limit: history_limit.max(2),
            host_cpu_ticks: None,
            host_process_ticks: HashMap::new(),
            host_network_bytes: HashMap::new(),
            gpu_adapter: None,
            gpu_adapter_checked: false,
            last_gpu_refresh: None,
        }
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
        let sample = StateSample {
            value: value.clone(),
            observed_at,
        };
        let metric = self
            .metrics
            .entry(key.clone())
            .or_insert_with(|| StateMetric {
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
            let prefix =
                format!("{:?}.{}", node.node_type, sanitize(&node.node_id.0)).to_ascii_lowercase();
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
                self.publish(
                    "cpu.load_1m",
                    "cpu:all",
                    load1,
                    None,
                    observed_at,
                    Some(observed_at + 15),
                    "procfs",
                    DataClassification::SystemConfig,
                );
            }
        }
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for key in ["MemTotal", "MemAvailable"] {
                if let Some(value) = meminfo
                    .lines()
                    .find_map(|line| line.strip_prefix(&format!("{key}:")))
                    .and_then(|value| value.split_whitespace().next())
                {
                    self.publish(
                        format!("memory.{}", sanitize(key)),
                        "memory:system",
                        value,
                        Some("kB".into()),
                        observed_at,
                        Some(observed_at + 15),
                        "procfs",
                        DataClassification::SystemConfig,
                    );
                }
            }
        }
        if let Ok(network) = std::fs::read_to_string("/proc/net/dev") {
            for line in network.lines().skip(2) {
                let Some((interface, counters)) = line.split_once(':') else {
                    continue;
                };
                let values: Vec<&str> = counters.split_whitespace().collect();
                if values.len() < 9 {
                    continue;
                }
                let interface = sanitize(interface.trim());
                let Ok(rx_bytes) = values[0].parse::<u64>() else {
                    continue;
                };
                let Ok(tx_bytes) = values[8].parse::<u64>() else {
                    continue;
                };
                let resource = format!("network:{interface}");
                self.publish(
                    format!("network.{interface}.rx_bytes"),
                    resource.clone(),
                    rx_bytes.to_string(),
                    Some("B".into()),
                    observed_at,
                    Some(observed_at + 15),
                    "procfs",
                    DataClassification::SystemConfig,
                );
                self.publish(
                    format!("network.{interface}.tx_bytes"),
                    resource.clone(),
                    tx_bytes.to_string(),
                    Some("B".into()),
                    observed_at,
                    Some(observed_at + 15),
                    "procfs",
                    DataClassification::SystemConfig,
                );
                if let Some((previous_rx, previous_tx, previous_at)) = self
                    .host_network_bytes
                    .insert(interface.clone(), (rx_bytes, tx_bytes, observed_at))
                {
                    let elapsed = observed_at.saturating_sub(previous_at).max(1);
                    self.publish(
                        format!("network.{interface}.rx_bps"),
                        resource.clone(),
                        (rx_bytes.saturating_sub(previous_rx) / elapsed).to_string(),
                        Some("B/s".into()),
                        observed_at,
                        Some(observed_at + 15),
                        "procfs",
                        DataClassification::SystemConfig,
                    );
                    self.publish(
                        format!("network.{interface}.tx_bps"),
                        resource,
                        (tx_bytes.saturating_sub(previous_tx) / elapsed).to_string(),
                        Some("B/s".into()),
                        observed_at,
                        Some(observed_at + 15),
                        "procfs",
                        DataClassification::SystemConfig,
                    );
                }
            }
        }
        self.refresh_process_cpu(observed_at);
        self.refresh_thermal(observed_at);
        self.refresh_gpu(observed_at);
    }

    #[cfg(not(target_os = "linux"))]
    pub fn refresh_host(&mut self) {}

    #[cfg(target_os = "linux")]
    fn refresh_process_cpu(&mut self, observed_at: Timestamp) {
        let Some((total_ticks, busy_ticks, cores)) = proc_cpu_totals() else {
            return;
        };
        let previous_cpu = self.host_cpu_ticks.replace((total_ticks, busy_ticks));
        let current = proc_process_ticks();
        let Some((previous_total, previous_busy)) = previous_cpu else {
            self.host_process_ticks = current;
            return;
        };
        let total_delta = total_ticks.saturating_sub(previous_total);
        if total_delta == 0 {
            return;
        }
        let utilization =
            busy_ticks.saturating_sub(previous_busy) as f64 * 100.0 / total_delta as f64;
        self.publish(
            "cpu.utilization_percent",
            "cpu:all",
            format!("{utilization:.2}"),
            Some("%".into()),
            observed_at,
            Some(observed_at + 15),
            "procfs",
            DataClassification::SystemConfig,
        );
        let mut ranked = current
            .iter()
            .filter_map(|(pid, ticks)| {
                let previous = self.host_process_ticks.get(pid).copied()?;
                let delta = ticks.saturating_sub(previous);
                (delta > 0).then_some((
                    *pid,
                    delta as f64 * cores as f64 * 100.0 / total_delta as f64,
                ))
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (pid, percent) in ranked.into_iter().take(32) {
            self.publish(
                format!("process.{pid}.cpu_percent"),
                format!("process:{pid}"),
                format!("{percent:.2}"),
                Some("%".into()),
                observed_at,
                Some(observed_at + 15),
                "procfs",
                DataClassification::SystemConfig,
            );
            if let Some(name) = proc_process_name(pid) {
                self.publish(
                    format!("process.{pid}.name"),
                    format!("process:{pid}"),
                    name,
                    None,
                    observed_at,
                    Some(observed_at + 15),
                    "procfs",
                    DataClassification::SystemConfig,
                );
            }
        }
        self.host_process_ticks = current;
    }

    #[cfg(target_os = "linux")]
    fn refresh_thermal(&mut self, observed_at: Timestamp) {
        let Ok(entries) = std::fs::read_dir("/sys/class/thermal") else {
            return;
        };
        for entry in entries.flatten() {
            let zone = entry.file_name().to_string_lossy().to_string();
            if !zone.starts_with("thermal_zone") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(entry.path().join("temp")) else {
                continue;
            };
            let Ok(milli_c) = raw.trim().parse::<f64>() else {
                continue;
            };
            let kind = std::fs::read_to_string(entry.path().join("type"))
                .unwrap_or_else(|_| "unknown".into());
            self.publish(
                format!(
                    "thermal.{}.{}.temperature_c",
                    sanitize(&zone),
                    sanitize(kind.trim())
                ),
                format!("thermal:{}", sanitize(&zone)),
                format!("{:.2}", milli_c / 1000.0),
                Some("C".into()),
                observed_at,
                Some(observed_at + 15),
                "sysfs",
                DataClassification::SystemConfig,
            );
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_gpu(&mut self, observed_at: Timestamp) {
        if !self.gpu_adapter_checked {
            self.gpu_adapter = GpuRuntimeAdapter::discover();
            self.gpu_adapter_checked = true;
        }
        if self
            .last_gpu_refresh
            .is_some_and(|previous| observed_at.saturating_sub(previous) < 5)
        {
            return;
        }
        let Some(adapter) = &self.gpu_adapter else {
            return;
        };
        if let Ok(samples) = adapter.collect() {
            self.ingest_gpu_samples(samples, observed_at);
            self.last_gpu_refresh = Some(observed_at);
        }
    }

    #[cfg(target_os = "linux")]
    fn ingest_gpu_samples(&mut self, samples: GpuRuntimeSamples, observed_at: Timestamp) {
        for device in samples.devices {
            let prefix = format!("gpu.{}", device.index);
            let resource = format!("gpu:{}", device.index);
            self.publish(
                format!("{prefix}.name"),
                resource.clone(),
                device.name,
                None,
                observed_at,
                Some(observed_at + 15),
                "nvidia_smi",
                DataClassification::SystemConfig,
            );
            for (suffix, value, unit) in [
                ("temperature_c", device.temperature_c, "C"),
                ("utilization_percent", device.utilization_percent, "%"),
                ("memory_used_mib", device.memory_used_mib, "MiB"),
                ("memory_total_mib", device.memory_total_mib, "MiB"),
                ("power_draw_w", device.power_draw_w, "W"),
            ] {
                if let Some(value) = value {
                    self.publish(
                        format!("{prefix}.{suffix}"),
                        resource.clone(),
                        format!("{value:.2}"),
                        Some(unit.into()),
                        observed_at,
                        Some(observed_at + 15),
                        "nvidia_smi",
                        DataClassification::SystemConfig,
                    );
                }
            }
        }
        for process in samples.processes {
            let Some(memory) = process.memory_used_mib else {
                continue;
            };
            self.publish(
                format!("process.{}.gpu_memory_mib", process.pid),
                format!("process:{}", process.pid),
                format!("{memory:.2}"),
                Some("MiB".into()),
                observed_at,
                Some(observed_at + 15),
                "nvidia_smi",
                DataClassification::SystemConfig,
            );
            self.publish(
                format!(
                    "gpu.{}.process.{}.memory_used_mib",
                    process.gpu_index, process.pid
                ),
                format!("gpu:{}", process.gpu_index),
                format!("{memory:.2}"),
                Some("MiB".into()),
                observed_at,
                Some(observed_at + 15),
                "nvidia_smi",
                DataClassification::SystemConfig,
            );
        }
    }

    pub fn metric(&self, key: &str) -> Option<&StateMetric> {
        self.metrics.get(key)
    }

    /// Return the current, non-stale values for a surface's explicitly
    /// declared projection keys. This is deliberately an exact-key API: a
    /// generated surface cannot discover arbitrary host state through fuzzy
    /// matching, and a stale/missing observation leaves its last valid value
    /// on screen instead of fabricating a replacement.
    pub fn binding_values<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a str>,
    ) -> BTreeMap<String, String> {
        let timestamp = now();
        keys.into_iter()
            .filter_map(|key| {
                let metric = self.metrics.get(key)?;
                (!metric.is_stale(timestamp)).then(|| (key.to_string(), metric.value.clone()))
            })
            .collect()
    }

    /// Build a presentation-only snapshot for exact declared binding keys.
    /// Fresh values and stale state are separate so the canvas can retain the
    /// last verified value while visibly marking it stale.
    pub fn binding_snapshot<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a str>,
    ) -> BindingSnapshot {
        let timestamp = now();
        let mut snapshot = BindingSnapshot::default();
        for key in keys {
            let Some(metric) = self.metrics.get(key) else {
                continue;
            };
            if metric.is_stale(timestamp) {
                snapshot.stale_keys.push(key.to_string());
            } else {
                snapshot.values.insert(key.to_string(), metric.value.clone());
            }
        }
        snapshot
    }

    pub fn project(&self, query: &str, limit: usize) -> ContextProjection {
        let timestamp = now();
        let terms = query_terms(query);
        let mut candidates: Vec<&StateMetric> = self
            .metrics
            .values()
            .filter(|metric| metric.classification != DataClassification::Protected)
            .collect();
        let query_lower = query.to_ascii_lowercase();
        let rank_process_cpu = terms.iter().any(|term| term.starts_with("process"))
            && (query_lower.contains("cpu")
                || terms
                    .iter()
                    .any(|term| term == "utilization" || term == "using"));
        candidates.sort_by_key(|metric| {
            let haystack = format!("{} {} {}", metric.key, metric.resource, metric.source)
                .to_ascii_lowercase();
            let matches = terms
                .iter()
                .filter(|term| haystack.contains(term.as_str()))
                .count();
            (std::cmp::Reverse(matches), metric.key.clone())
        });
        if rank_process_cpu {
            candidates.retain(|metric| {
                metric.key.starts_with("process.") && metric.key.ends_with(".cpu_percent")
            });
            candidates.sort_by(|left, right| {
                right
                    .value
                    .parse::<f64>()
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(&left.value.parse::<f64>().unwrap_or(f64::NEG_INFINITY))
                    .then_with(|| left.key.cmp(&right.key))
            });
        }
        if !terms.is_empty()
            && candidates.iter().any(|metric| {
                let haystack = format!("{} {}", metric.key, metric.resource).to_ascii_lowercase();
                terms.iter().any(|term| haystack.contains(term.as_str()))
            })
        {
            candidates.retain(|metric| {
                let haystack = format!("{} {}", metric.key, metric.resource).to_ascii_lowercase();
                terms.iter().any(|term| haystack.contains(term.as_str()))
            });
        }
        let metric_limit = if rank_process_cpu {
            requested_top_count(query)
                .unwrap_or(limit.saturating_div(2).max(1))
                .min(limit.saturating_div(2).max(1))
        } else {
            limit
        };
        let truncated = candidates.len() > metric_limit;
        let selected: Vec<&StateMetric> = candidates.into_iter().take(metric_limit).collect();
        let mut facts: Vec<ContextFact> = selected
            .iter()
            .map(|metric| ContextFact {
                key: metric.key.clone(),
                value: metric.value.clone(),
                unit: metric.unit.clone(),
                resource: metric.resource.clone(),
                source: metric.source.clone(),
                observed_at: metric.observed_at,
                freshness: if metric.is_stale(timestamp) {
                    "stale".into()
                } else {
                    "fresh".into()
                },
            })
            .collect();
        if rank_process_cpu {
            for metric in selected {
                if facts.len() >= limit {
                    break;
                }
                let Some(prefix) = metric.key.strip_suffix(".cpu_percent") else {
                    continue;
                };
                let Some(name) = self.metrics.get(&format!("{prefix}.name")) else {
                    continue;
                };
                facts.push(ContextFact {
                    key: name.key.clone(),
                    value: name.value.clone(),
                    unit: None,
                    resource: name.resource.clone(),
                    source: name.source.clone(),
                    observed_at: name.observed_at,
                    freshness: if name.is_stale(timestamp) {
                        "stale".into()
                    } else {
                        "fresh".into()
                    },
                });
            }
        }
        let findings = self
            .findings(query)
            .into_iter()
            .take(limit.saturating_div(4).max(1))
            .collect();
        ContextProjection {
            generated_at: timestamp,
            query: query.into(),
            facts,
            findings,
            truncated,
        }
    }

    pub fn findings(&self, query: &str) -> Vec<StateFinding> {
        let terms = query_terms(query);
        let timestamp = now();
        let mut findings = self
            .metrics
            .values()
            .filter_map(|metric| {
                if metric.is_stale(timestamp) {
                    return None;
                }
                if !terms.is_empty()
                    && !terms
                        .iter()
                        .any(|term| metric.key.to_ascii_lowercase().contains(term))
                {
                    return None;
                }
                let first = metric.history.front()?;
                let last = metric.history.back()?;
                if first.observed_at == last.observed_at {
                    return None;
                }
                let from = first.value.parse::<f64>().ok()?;
                let to = last.value.parse::<f64>().ok()?;
                let delta = to - from;
                if delta.abs() < f64::EPSILON {
                    return None;
                }
                Some(StateFinding {
                    key: metric.key.clone(),
                    kind: if delta > 0.0 {
                        "rising".into()
                    } else {
                        "falling".into()
                    },
                    detail: format!("{} changed by {:+.2} between samples", metric.key, delta),
                    observed_from: first.observed_at,
                    observed_to: last.observed_at,
                    source_keys: vec![metric.key.clone()],
                    confidence_rule: "numeric change across bounded retained samples".into(),
                    freshness: "fresh".into(),
                })
            })
            .collect::<Vec<_>>();
        findings.extend(self.gpu_network_correlations(&terms, timestamp));
        findings
    }

    fn gpu_network_correlations(
        &self,
        terms: &[String],
        timestamp: Timestamp,
    ) -> Vec<StateFinding> {
        let asks_about_gpu = terms
            .iter()
            .any(|term| matches!(term.as_str(), "gpu" | "graphics" | "temperature"));
        let asks_about_network = terms.iter().any(|term| {
            matches!(
                term.as_str(),
                "network" | "wifi" | "lan" | "bandwidth" | "traffic"
            )
        });
        if !asks_about_gpu || !asks_about_network {
            return Vec::new();
        }
        let network = self.metrics.values().filter(|metric| {
            !metric.is_stale(timestamp)
                && metric.key.starts_with("network.")
                && (metric.key.ends_with(".rx_bps") || metric.key.ends_with(".tx_bps"))
                && !metric.key.starts_with("network.lo.")
                && metric.value.parse::<f64>().is_ok_and(|value| value > 0.0)
        });
        let mut findings = Vec::new();
        for temperature in self.metrics.values().filter(|metric| {
            !metric.is_stale(timestamp)
                && metric.key.starts_with("gpu.")
                && metric.key.ends_with(".temperature_c")
        }) {
            let (Some(first), Some(last)) =
                (temperature.history.front(), temperature.history.back())
            else {
                continue;
            };
            let (Ok(from), Ok(to)) = (first.value.parse::<f64>(), last.value.parse::<f64>()) else {
                continue;
            };
            let increase = to - from;
            if first.observed_at == last.observed_at || increase < 2.0 {
                continue;
            }
            let Some(gpu_index) = temperature
                .key
                .strip_prefix("gpu.")
                .and_then(|key| key.split_once('.'))
                .map(|(index, _)| index)
            else {
                continue;
            };
            let process_prefix = format!("gpu.{gpu_index}.process.");
            for process in self.metrics.values().filter(|metric| {
                !metric.is_stale(timestamp)
                    && metric.key.starts_with(&process_prefix)
                    && metric.key.ends_with(".memory_used_mib")
                    && metric.value.parse::<f64>().is_ok_and(|value| value > 0.0)
            }) {
                let Some(pid) = process
                    .key
                    .strip_prefix(&process_prefix)
                    .and_then(|key| key.strip_suffix(".memory_used_mib"))
                else {
                    continue;
                };
                for traffic in network.clone() {
                    let earliest = first
                        .observed_at
                        .min(process.observed_at)
                        .min(traffic.observed_at);
                    let latest = last
                        .observed_at
                        .max(process.observed_at)
                        .max(traffic.observed_at);
                    if latest.saturating_sub(earliest) > 15 {
                        continue;
                    }
                    findings.push(StateFinding {
                        key: format!("correlation.{}.{}.{}", temperature.key, pid, traffic.key),
                        kind: "temporal_overlap".into(),
                        detail: format!(
                            "{} rose {increase:+.2} C while process {pid} held GPU memory and {} carried {} B/s; this is temporal correlation, not causation",
                            temperature.key, traffic.key, traffic.value,
                        ),
                        observed_from: earliest,
                        observed_to: latest,
                        source_keys: vec![temperature.key.clone(), process.key.clone(), traffic.key.clone()],
                        confidence_rule: "GPU temperature rose by at least 2 C across retained samples; nonzero GPU memory and host-interface throughput were observed within 15 seconds".into(),
                        freshness: "fresh".into(),
                    });
                    if findings.len() >= 4 {
                        return findings;
                    }
                }
            }
        }
        findings
    }
}

fn query_terms(query: &str) -> Vec<String> {
    const GENERIC_TERMS: &[&str] = &[
        "about",
        "display",
        "displaying",
        "generate",
        "please",
        "show",
        "status",
        "surface",
        "usage",
        "using",
        "with",
    ];
    query
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|term| term.len() >= 3 && !GENERIC_TERMS.contains(term))
        .map(str::to_string)
        .collect()
}

fn requested_top_count(query: &str) -> Option<usize> {
    let lowercase = query.to_ascii_lowercase();
    let mut previous_was_top = false;
    for word in lowercase.split(|c: char| !c.is_ascii_alphanumeric()) {
        if previous_was_top {
            if let Ok(count) = word.parse::<usize>() {
                return Some(count.max(1));
            }
        }
        previous_was_top = word == "top";
    }
    None
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn health_name(health: &HealthState) -> &'static str {
    match health {
        HealthState::Healthy => "healthy",
        HealthState::Degraded => "degraded",
        HealthState::Unhealthy => "unhealthy",
        HealthState::Unknown => "unknown",
        HealthState::Stale => "stale",
    }
}

#[cfg(target_os = "linux")]
fn proc_cpu_totals() -> Option<(u64, u64, usize)> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let values = stat
        .lines()
        .find(|line| line.starts_with("cpu "))?
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();
    let total = values.iter().sum();
    let idle = values
        .get(3)
        .copied()
        .unwrap_or(0)
        .saturating_add(values.get(4).copied().unwrap_or(0));
    let cores = stat
        .lines()
        .filter(|line| {
            line.starts_with("cpu") && line.as_bytes().get(3).is_some_and(u8::is_ascii_digit)
        })
        .count()
        .max(1);
    Some((total, total.saturating_sub(idle), cores))
}

#[cfg(target_os = "linux")]
fn proc_process_ticks() -> HashMap<u32, u64> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return HashMap::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_string_lossy().parse::<u32>().ok()?;
            let stat = std::fs::read_to_string(entry.path().join("stat")).ok()?;
            let close = stat.rfind(')')?;
            let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
            Some((
                pid,
                fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?,
            ))
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn proc_process_name(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_bounded_history_and_reports_non_causal_trend() {
        let mut state = SystemStateStore::with_history_limit(2);
        state.publish(
            "gpu.0.temperature_c",
            "gpu:0",
            "70",
            Some("C".into()),
            10,
            None,
            "nvml",
            DataClassification::SystemConfig,
        );
        state.publish(
            "gpu.0.temperature_c",
            "gpu:0",
            "82",
            Some("C".into()),
            20,
            None,
            "nvml",
            DataClassification::SystemConfig,
        );
        let findings = state.findings("gpu temperature");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "rising");
        assert!(findings[0].detail.contains("+12.00"));
    }

    #[test]
    fn projection_is_relevant_and_bounded() {
        let mut state = SystemStateStore::new();
        state.publish(
            "cpu.0.utilization",
            "cpu:0",
            "54",
            Some("%".into()),
            10,
            None,
            "procfs",
            DataClassification::SystemConfig,
        );
        state.publish(
            "network.eth0.rx_bps",
            "device:net-eth0",
            "99",
            None,
            10,
            None,
            "procfs",
            DataClassification::SystemConfig,
        );
        let projection = state.project("", 1);
        assert_eq!(projection.facts.len(), 1);
        assert!(projection.facts[0].key.contains("cpu"));
        assert!(projection.truncated);
    }

    #[test]
    fn presentation_words_do_not_outweigh_the_requested_domain() {
        let mut state = SystemStateStore::new();
        state.publish(
            "cpu.load_1m",
            "cpu:all",
            "1.0",
            None,
            10,
            None,
            "procfs",
            DataClassification::SystemConfig,
        );
        state.publish(
            "filesystem.root.usage_used_percent",
            "filesystem:/",
            "70",
            Some("%".into()),
            10,
            None,
            "procfs",
            DataClassification::SystemConfig,
        );
        let projection = state.project("generate a surface displaying cpu usage please", 8);
        assert_eq!(projection.facts.len(), 1);
        assert_eq!(projection.facts[0].key, "cpu.load_1m");
    }

    #[test]
    fn process_cpu_projection_is_ranked_and_pairs_the_process_name() {
        let mut state = SystemStateStore::new();
        state.publish(
            "process.1.cpu_percent",
            "process:1",
            "3.5",
            Some("%".into()),
            10,
            None,
            "procfs",
            DataClassification::SystemConfig,
        );
        state.publish(
            "process.1.name",
            "process:1",
            "init",
            None,
            10,
            None,
            "procfs",
            DataClassification::SystemConfig,
        );
        state.publish(
            "process.2.cpu_percent",
            "process:2",
            "42.0",
            Some("%".into()),
            10,
            None,
            "procfs",
            DataClassification::SystemConfig,
        );
        state.publish(
            "process.2.name",
            "process:2",
            "renderer",
            None,
            10,
            None,
            "procfs",
            DataClassification::SystemConfig,
        );
        let projection = state.project("show the top 1 processes using cpu", 8);
        assert_eq!(projection.facts[0].key, "process.2.cpu_percent");
        assert_eq!(projection.facts[1].key, "process.2.name");
    }

    #[test]
    fn binding_values_are_exact_and_never_return_stale_metrics() {
        let mut state = SystemStateStore::new();
        let timestamp = now();
        state.publish(
            "cpu.utilization_percent",
            "cpu:all",
            "44.2",
            Some("%".into()),
            timestamp,
            Some(timestamp + 60),
            "procfs",
            DataClassification::SystemConfig,
        );
        state.publish(
            "cpu.stale",
            "cpu:all",
            "old",
            None,
            1,
            Some(1),
            "procfs",
            DataClassification::SystemConfig,
        );
        let values = state.binding_values(["cpu.utilization_percent", "cpu_stale", "cpu.stale"]);
        assert_eq!(
            values.get("cpu.utilization_percent").map(String::as_str),
            Some("44.2")
        );
        assert!(!values.contains_key("cpu_stale"));
        assert!(!values.contains_key("cpu.stale"));
    }

    #[test]
    fn reports_bounded_gpu_process_network_overlap_without_claiming_causation() {
        let mut state = SystemStateStore::with_history_limit(4);
        let timestamp = now();
        for (value, observed_at) in [(70.0, timestamp - 10), (73.0, timestamp)] {
            state.publish(
                "gpu.0.temperature_c",
                "gpu:0",
                format!("{value:.2}"),
                Some("C".into()),
                observed_at,
                Some(timestamp + 30),
                "nvidia_smi",
                DataClassification::SystemConfig,
            );
        }
        state.publish(
            "process.4242.gpu_memory_mib",
            "process:4242",
            "512.00",
            Some("MiB".into()),
            timestamp,
            Some(timestamp + 30),
            "nvidia_smi",
            DataClassification::SystemConfig,
        );
        state.publish(
            "gpu.0.process.4242.memory_used_mib",
            "gpu:0",
            "512.00",
            Some("MiB".into()),
            timestamp,
            Some(timestamp + 30),
            "nvidia_smi",
            DataClassification::SystemConfig,
        );
        state.publish(
            "network.wlan0.tx_bps",
            "network:wlan0",
            "4096",
            Some("B/s".into()),
            timestamp,
            Some(timestamp + 30),
            "procfs",
            DataClassification::SystemConfig,
        );

        let finding = state
            .findings("GPU temperature and WiFi bandwidth")
            .into_iter()
            .find(|finding| finding.kind == "temporal_overlap")
            .expect("a bounded temporal overlap");
        assert!(finding.detail.contains("not causation"));
        assert_eq!(finding.source_keys.len(), 3);
        assert!(finding.confidence_rule.contains("15 seconds"));
        assert_eq!(finding.freshness, "fresh");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ingests_gpu_runtime_samples_using_stable_projection_keys() {
        let mut state = SystemStateStore::new();
        let observed_at = now();
        state.ingest_gpu_samples(
            GpuRuntimeSamples {
                devices: vec![crate::gpu_runtime::GpuDeviceSample {
                    index: 0,
                    uuid: "GPU-test".into(),
                    name: "Test GPU".into(),
                    temperature_c: Some(71.0),
                    utilization_percent: Some(82.0),
                    memory_used_mib: Some(2048.0),
                    memory_total_mib: Some(8192.0),
                    power_draw_w: Some(125.5),
                }],
                processes: vec![crate::gpu_runtime::GpuProcessSample {
                    gpu_index: 0,
                    pid: 4242,
                    memory_used_mib: Some(512.0),
                }],
            },
            observed_at,
        );
        assert_eq!(state.metric("gpu.0.temperature_c").unwrap().value, "71.00");
        assert_eq!(
            state.metric("process.4242.gpu_memory_mib").unwrap().value,
            "512.00"
        );
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
