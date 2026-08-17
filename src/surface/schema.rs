//! surface/v1 schema: the typed contract between the AI composition layer and
//! the renderer.
//!
//! The model describes the surface; it never renders it. `Surface` carries
//! semantic layout (regions, spans, priority) plus a closed widget vocabulary,
//! and every widget binds its values to evidence keys (see `evidence` /
//! `validator` in later phases) so a widget can never display a value the
//! specialists did not return.
//!
//! Field names serialize as camelCase to match the frontend; widget `type`
//! tags are internally tagged and camelCased (`metricCard`, `sensorGauge`,
//! `statusList`, `chart`, `notice`).

use serde::{Deserialize, Serialize};

/// Version of the surface contract. Bump on any incompatible change to the
/// widget vocabulary or layout grammar; the renderer may key behavior on this.
pub const SURFACE_VERSION: u32 = 1;

/// A complete, validated generative surface (surface/v1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Surface {
    /// Echo of the user intent that produced this surface.
    pub intent: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// Where the panel should sit on the desktop (Phase G). Semantic hints
    /// only; the model never emits pixels.
    pub placement: SurfacePlacement,
    pub layout: SurfaceLayout,
    pub regions: Vec<SurfaceRegion>,
    pub widgets: Vec<SurfaceWidget>,
}

/// Top-level arrangement. `Grid` gives each region `span` of `columns`;
/// `Stack` renders regions vertically; `Row` renders them horizontally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceLayout {
    #[serde(default)]
    pub mode: LayoutMode,
    /// Grid columns; default 12.
    #[serde(default = "default_columns")]
    pub columns: u32,
    #[serde(default)]
    pub density: SurfaceDensity,
}

fn default_columns() -> u32 {
    12
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    Grid,
    Stack,
    Row,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SurfaceDensity {
    Compact,
    Comfortable,
    Detailed,
}

impl Default for SurfaceDensity {
    fn default() -> Self {
        SurfaceDensity::Comfortable
    }
}

impl Default for LayoutMode {
    fn default() -> Self {
        LayoutMode::Grid
    }
}

/// A named area of the surface holding one or more widgets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRegion {
    pub id: String,
    /// Grid columns consumed by this region (grid mode).
    pub span: u32,
    pub priority: RegionPriority,
    /// Widget ids rendered inside this region, in order.
    pub widgets: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionPriority {
    Primary,
    Secondary,
    Tertiary,
}

/// Closed widget vocabulary for v0.1. Every value is bound to one or more
/// evidence keys so the validator can prove it came from the specialists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SurfaceWidget {
    MetricCard {
        id: String,
        title: String,
        value: String,
        unit: Option<String>,
        status: Option<String>,
        evidence: Vec<String>,
    },
    SensorGauge {
        id: String,
        title: String,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
        unit: Option<String>,
        evidence: Vec<String>,
    },
    StatusList {
        id: String,
        title: String,
        items: Vec<StatusItem>,
        evidence: Vec<String>,
    },
    Chart {
        id: String,
        title: String,
        data: Vec<ChartPoint>,
        evidence: Vec<String>,
    },
    Notice {
        id: String,
        title: String,
        body: String,
        evidence: Vec<String>,
    },
}

impl SurfaceWidget {
    /// The widget's stable id, referenced by regions.
    pub fn id(&self) -> &str {
        match self {
            SurfaceWidget::MetricCard { id, .. }
            | SurfaceWidget::SensorGauge { id, .. }
            | SurfaceWidget::StatusList { id, .. }
            | SurfaceWidget::Chart { id, .. }
            | SurfaceWidget::Notice { id, .. } => id,
        }
    }

    /// Evidence keys this widget's values are bound to.
    pub fn evidence(&self) -> &[String] {
        match self {
            SurfaceWidget::MetricCard { evidence, .. }
            | SurfaceWidget::SensorGauge { evidence, .. }
            | SurfaceWidget::StatusList { evidence, .. }
            | SurfaceWidget::Chart { evidence, .. }
            | SurfaceWidget::Notice { evidence, .. } => evidence,
        }
    }
}

/// One row in a status list widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusItem {
    pub label: String,
    pub status: String,
    pub detail: Option<String>,
}

/// One bar in a chart widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

/// Where the panel should sit. Pure hints for the window layer; the composer
/// may express an edge and a width class, never coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfacePlacement {
    #[serde(default)]
    pub edge: Option<DockEdge>,
    #[serde(default)]
    pub width: Option<WidthClass>,
    /// True when the panel floats freely; docked only when `edge` is set.
    #[serde(default = "default_float")]
    pub float: bool,
}

fn default_float() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WidthClass {
    Narrow,
    Medium,
    Wide,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_surface() -> Surface {
        Surface {
            intent: "How healthy is the storage subsystem?".to_string(),
            title: "Storage Health".to_string(),
            subtitle: Some("Current machine state".to_string()),
            placement: SurfacePlacement {
                edge: None,
                width: None,
                float: true,
            },
            layout: SurfaceLayout {
                mode: LayoutMode::Grid,
                columns: 12,
                density: SurfaceDensity::Comfortable,
            },
            regions: vec![
                SurfaceRegion {
                    id: "health".to_string(),
                    span: 8,
                    priority: RegionPriority::Primary,
                    widgets: vec!["storage-hero".to_string()],
                },
                SurfaceRegion {
                    id: "thermal".to_string(),
                    span: 4,
                    priority: RegionPriority::Secondary,
                    widgets: vec!["disk-temp".to_string()],
                },
            ],
            widgets: vec![
                SurfaceWidget::MetricCard {
                    id: "storage-hero".to_string(),
                    title: "Overall Health".to_string(),
                    value: "Healthy".to_string(),
                    unit: None,
                    status: Some("healthy".to_string()),
                    evidence: vec!["tool-0".to_string()],
                },
                SurfaceWidget::SensorGauge {
                    id: "disk-temp".to_string(),
                    title: "Disk Temperature".to_string(),
                    value: 63.0,
                    min: Some(0.0),
                    max: Some(100.0),
                    unit: Some("C".to_string()),
                    evidence: vec!["tool-1".to_string()],
                },
            ],
        }
    }

    fn surface_with_every_widget() -> Surface {
        let mut surface = example_surface();
        surface.widgets.push(SurfaceWidget::StatusList {
            id: "checks".to_string(),
            title: "Specialist checks".to_string(),
            items: vec![StatusItem {
                label: "SMART".to_string(),
                status: "Healthy".to_string(),
                detail: Some("no faults".to_string()),
            }],
            evidence: vec!["tool-0".to_string()],
        });
        surface.widgets.push(SurfaceWidget::Chart {
            id: "history".to_string(),
            title: "Usage history".to_string(),
            data: vec![
                ChartPoint {
                    label: "1h".to_string(),
                    value: 12.0,
                },
                ChartPoint {
                    label: "2h".to_string(),
                    value: 31.0,
                },
            ],
            evidence: vec!["tool-0".to_string()],
        });
        surface.widgets.push(SurfaceWidget::Notice {
            id: "note".to_string(),
            title: "Stale evidence".to_string(),
            body: "Last scan was 4 hours ago.".to_string(),
            evidence: vec!["tool-0".to_string()],
        });
        surface
    }

    #[test]
    fn round_trips_a_valid_surface() {
        let surface = example_surface();
        let json = serde_json::to_string(&surface).expect("serialize");
        let back: Surface = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(surface, back);
    }

    #[test]
    fn round_trips_every_widget_variant() {
        let surface = surface_with_every_widget();
        let json = serde_json::to_string(&surface).expect("serialize");
        let back: Surface = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(surface, back);
        assert_eq!(back.widgets.len(), 5);
    }

    #[test]
    fn serializes_with_camelcase_fields_and_type_tags() {
        let surface = surface_with_every_widget();
        let json = serde_json::to_string(&surface).expect("serialize");
        assert!(json.contains("\"type\":\"metricCard\""), "{json}");
        assert!(json.contains("\"type\":\"sensorGauge\""), "{json}");
        assert!(json.contains("\"type\":\"statusList\""), "{json}");
        assert!(json.contains("\"type\":\"chart\""), "{json}");
        assert!(json.contains("\"type\":\"notice\""), "{json}");
        assert!(json.contains("\"intent\":"), "{json}");
    }

    #[test]
    fn placement_serializes_with_semantic_values() {
        let placement = SurfacePlacement {
            edge: Some(DockEdge::Right),
            width: Some(WidthClass::Narrow),
            float: false,
        };
        let json = serde_json::to_string(&placement).expect("serialize");
        assert!(json.contains("\"edge\":\"right\""), "{json}");
        assert!(json.contains("\"width\":\"narrow\""), "{json}");
        assert!(json.contains("\"float\":false"), "{json}");
    }

    #[test]
    fn layout_mode_defaults_to_grid() {
        let layout: SurfaceLayout = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(layout.mode, LayoutMode::Grid);
        assert_eq!(layout.columns, 12);
        assert_eq!(layout.density, SurfaceDensity::Comfortable);
    }

    #[test]
    fn missing_required_field_fails_to_deserialize() {
        let surface = example_surface();
        let json = serde_json::to_string(&surface).expect("serialize");
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        value
            .as_object_mut()
            .expect("object")
            .remove("regions");
        let json = serde_json::to_string(&value).expect("serialize");
        let err = serde_json::from_str::<Surface>(&json).unwrap_err();
        assert!(
            err.to_string().contains("missing field `regions`"),
            "{err}"
        );
    }

    #[test]
    fn dangling_region_reference_deserializes_but_needs_validation() {
        // serde alone cannot catch a region pointing at a widget id that does
        // not exist. Phase D (`validator`) rejects this; the test documents
        // why validation cannot live in the schema types alone.
        let mut surface = example_surface();
        surface.regions[0].widgets.push("ghost".to_string());
        let json = serde_json::to_string(&surface).expect("serialize");
        let back: Surface = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.regions[0].widgets, vec!["storage-hero", "ghost"]);
    }

    #[test]
    fn widget_ids_are_addressable() {
        let surface = example_surface();
        assert_eq!(
            surface.widgets.iter().map(|w| w.id()).collect::<Vec<_>>(),
            vec!["storage-hero", "disk-temp"]
        );
    }
}
