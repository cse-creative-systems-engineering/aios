//! Durable lifecycle state for unconstrained A2UI surfaces (ADR-0012).
//!
//! This module deliberately knows nothing about visual components. It owns
//! stable identity, revision, placement, persistence-friendly metadata, and
//! the binding keys a generated fragment declares. Rendering remains entirely
//! the surface model's responsibility.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceLayout {
    pub x: f64,
    pub y: f64,
    pub z_index: u32,
    pub visible: bool,
}

impl Default for SurfaceLayout {
    fn default() -> Self {
        Self {
            x: 48.0,
            y: 44.0,
            z_index: 1,
            visible: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRecord {
    pub id: String,
    pub revision: u64,
    pub intent: String,
    pub html: String,
    pub layout: SurfaceLayout,
    pub bindings: Vec<String>,
    /// Last delivered values for exact projection-key bindings. This is data
    /// state, not a visual revision: live samples do not regenerate HTML.
    #[serde(default)]
    pub binding_values: BTreeMap<String, String>,
    #[serde(default)]
    pub data_revision: u64,
}

/// Incremental, read-only replacement values for one existing surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceDelta {
    pub id: String,
    pub revision: u64,
    pub data_revision: u64,
    pub values: BTreeMap<String, String>,
}

impl SurfaceRecord {
    pub fn new(id: String, intent: String, html: String, layout: SurfaceLayout) -> Self {
        let bindings = declared_bindings(&html);
        Self {
            id,
            revision: 1,
            intent,
            html,
            layout,
            bindings,
            binding_values: BTreeMap::new(),
            data_revision: 0,
        }
    }

    /// Accept a model-authored revision only after the caller has applied its
    /// fidelity and policy gates. Existing layout stays stable by default.
    pub fn revise(&mut self, intent: String, html: String) {
        self.revision = self.revision.saturating_add(1);
        self.intent = intent;
        self.bindings = declared_bindings(&html);
        self.binding_values
            .retain(|key, _| self.bindings.contains(key));
        self.html = html;
    }

    pub fn set_initial_binding_values(&mut self, values: BTreeMap<String, String>) {
        self.binding_values = values
            .into_iter()
            .filter(|(key, _)| self.bindings.contains(key))
            .collect();
    }
}

#[derive(Clone, Debug, Default)]
pub struct SurfaceRuntime {
    surfaces: Vec<SurfaceRecord>,
}

impl SurfaceRuntime {
    pub fn restore(surfaces: Vec<SurfaceRecord>) -> Self {
        Self { surfaces }
    }

    pub fn open(&mut self, surface: SurfaceRecord) {
        self.surfaces.retain(|existing| existing.id != surface.id);
        self.surfaces.push(surface);
    }

    /// Backend-assigned initial placement keeps the lifecycle authoritative
    /// while still giving newly generated cards a discoverable cascade.
    pub fn next_layout(&self) -> SurfaceLayout {
        let offset = ((self.surfaces.len() % 8) + 1) as f64 * 28.0;
        SurfaceLayout {
            x: 20.0 + offset,
            y: 16.0 + offset,
            z_index: self.surfaces.len() as u32 + 1,
            visible: true,
        }
    }

    pub fn close(&mut self, id: &str) -> bool {
        let before = self.surfaces.len();
        self.surfaces.retain(|surface| surface.id != id);
        self.surfaces.len() != before
    }

    pub fn set_layout(&mut self, id: &str, layout: SurfaceLayout) -> Result<(), String> {
        let surface = self
            .surfaces
            .iter_mut()
            .find(|surface| surface.id == id)
            .ok_or_else(|| format!("no surface '{id}' is open"))?;
        surface.layout = layout;
        Ok(())
    }

    pub fn all(&self) -> &[SurfaceRecord] {
        &self.surfaces
    }

    /// Apply exact binding values without changing the model-authored HTML,
    /// layout, or visual revision. Only changed, declared keys become a delta.
    pub fn apply_binding_values(&mut self, values: &BTreeMap<String, String>) -> Vec<SurfaceDelta> {
        self.surfaces
            .iter_mut()
            .filter_map(|surface| {
                let changed = surface
                    .bindings
                    .iter()
                    .filter_map(|key| {
                        let value = values.get(key)?;
                        (surface.binding_values.get(key) != Some(value))
                            .then(|| (key.clone(), value.clone()))
                    })
                    .collect::<BTreeMap<_, _>>();
                if changed.is_empty() {
                    return None;
                }
                surface.binding_values.extend(changed.clone());
                surface.data_revision = surface.data_revision.saturating_add(1);
                Some(SurfaceDelta {
                    id: surface.id.clone(),
                    revision: surface.revision,
                    data_revision: surface.data_revision,
                    values: changed,
                })
            })
            .collect()
    }
}

/// Extract the stable, read-only projection keys declared by generated HTML.
/// This is intentionally a small attribute scanner, rather than an HTML
/// rewriter: no generated script runs and the runtime never dictates markup.
pub fn declared_bindings(html: &str) -> Vec<String> {
    let mut bindings = Vec::new();
    let mut remaining = html;
    while let Some(start) = remaining.find("data-aios=\"") {
        let value_start = start + "data-aios=\"".len();
        let after = &remaining[value_start..];
        let Some(end) = after.find('"') else {
            break;
        };
        let key = &after[..end];
        if !key.is_empty() && !bindings.iter().any(|known| known == key) {
            bindings.push(key.to_string());
        }
        remaining = &after[end + 1..];
    }
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_identity_revision_layout_and_declared_bindings() {
        let mut record = SurfaceRecord::new(
            "surface-1".into(),
            "show CPU".into(),
            r#"<div><span data-aios="cpu.total">12</span></div>"#.into(),
            SurfaceLayout::default(),
        );
        assert_eq!(record.bindings, vec!["cpu.total"]);
        record.revise(
            "make it yellow".into(),
            r#"<b data-aios="cpu.total">13</b>"#.into(),
        );
        assert_eq!(record.revision, 2);
        assert_eq!(record.layout.x, 48.0);
    }

    #[test]
    fn runtime_persists_layout_across_lifecycle_operations() {
        let mut runtime = SurfaceRuntime::default();
        runtime.open(SurfaceRecord::new(
            "surface-1".into(),
            "cpu".into(),
            "<div/>".into(),
            SurfaceLayout::default(),
        ));
        runtime
            .set_layout(
                "surface-1",
                SurfaceLayout {
                    x: 10.0,
                    y: 20.0,
                    z_index: 4,
                    visible: true,
                },
            )
            .unwrap();
        assert_eq!(runtime.all()[0].layout.z_index, 4);
        assert!(runtime.close("surface-1"));
    }

    #[test]
    fn live_values_emit_a_delta_without_revising_or_replacing_html() {
        let mut runtime = SurfaceRuntime::default();
        let mut surface = SurfaceRecord::new(
            "surface-1".into(),
            "cpu".into(),
            r#"<span data-aios="cpu.utilization_percent">10</span>"#.into(),
            SurfaceLayout::default(),
        );
        surface.set_initial_binding_values(BTreeMap::from([(
            "cpu.utilization_percent".into(),
            "10".into(),
        )]));
        runtime.open(surface);
        let updates = runtime.apply_binding_values(&BTreeMap::from([(
            "cpu.utilization_percent".into(),
            "77.77".into(),
        )]));
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].revision, 1);
        assert_eq!(updates[0].data_revision, 1);
        assert_eq!(updates[0].values["cpu.utilization_percent"], "77.77");
        assert_eq!(
            runtime.all()[0].html,
            r#"<span data-aios="cpu.utilization_percent">10</span>"#
        );
        assert_eq!(runtime.all()[0].revision, 1);
    }
}
