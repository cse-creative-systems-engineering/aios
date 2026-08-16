//! Surface validation (Phase D).
//!
//! The composer output is a model product: even a perfect schema parse can
//! carry a dangling widget reference, a fabricated measurement, or a widget
//! that no region displays. `validate` runs mechanical checks in order and
//! returns the first failure. The harness and (Phase E) the IPC layer use this
//! to decide whether a surface is trustworthy enough to render.
//!
//! Check levels:
//! - Hard errors (reject the surface): schema violations, evidence keys that
//!   do not exist, metric/gauge values that are not proven present in the
//!   referenced evidence, dangling or duplicate widget references, and
//!   unreferenced widgets.
//! - Soft diagnostics (harness-only warnings): statusList detail / chart point
//!   presence. Those values are allowed to be derived or reworded, so they are
//!   reported rather than rejected.

use crate::surface::evidence::{
    EvidenceIndex, number_present_in_evidence, value_present_in_evidence,
};
use crate::surface::schema::{LayoutMode, Surface, SurfaceWidget};

/// The first validation failure. `stage` names the check bucket so callers can
/// bucket errors (and the harness can report them per stage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub stage: &'static str,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.stage, self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Validate a surface against its evidence index. Returns the first failure.
pub fn validate(surface: &Surface, evidence: &EvidenceIndex) -> Result<(), ValidationError> {
    schema_check(surface)?;
    evidence_check(surface, evidence)?;
    layout_check(surface)?;
    Ok(())
}

fn schema_check(surface: &Surface) -> Result<(), ValidationError> {
    if surface.intent.trim().is_empty() {
        return Err(err("schema", "intent is empty"));
    }
    if surface.title.trim().is_empty() {
        return Err(err("schema", "title is empty"));
    }
    let columns = surface.layout.columns;
    if columns == 0 {
        return Err(err("schema", "layout.columns is 0"));
    }
    for region in &surface.regions {
        if region.id.trim().is_empty() {
            return Err(err("schema", "region has an empty id"));
        }
        if region.span == 0 {
            return Err(err("schema", format!("region '{}' has span 0", region.id)));
        }
        if region.span > columns {
            return Err(err("schema", format!("region '{}' span {} exceeds layout.columns {}", region.id, region.span, columns)));
        }
    }
    let mut seen = std::collections::HashSet::new();
    for widget in &surface.widgets {
        if widget.id().trim().is_empty() {
            return Err(err("schema", "widget has an empty id"));
        }
        if !seen.insert(widget.id().to_string()) {
            return Err(err("schema", format!("duplicate widget id '{}'", widget.id())));
        }
    }
    Ok(())
}

fn evidence_check(surface: &Surface, evidence: &EvidenceIndex) -> Result<(), ValidationError> {
    for widget in &surface.widgets {
        let keys = widget.evidence();
        if keys.is_empty() {
            return Err(err("evidence", format!("widget '{}' binds no evidence", widget.id())));
        }
        for key in keys {
            let Some(entry) = evidence.get(key) else {
                return Err(err("evidence", format!("widget '{}' references missing evidence key '{}'", widget.id(), key)));
            };
            if !bound_value_proven(widget, entry) {
                return Err(err("evidence", format!("widget '{}' value is not present in evidence '{}'", widget.id(), key)));
            }
        }
    }
    Ok(())
}

/// Hard value-binding proof for the primitive widgets. MetricCard and
/// SensorGauge display a single measurement, so its provenance is required.
/// StatusList/Chart/Notice are composite or advisory and are checked as soft
/// diagnostics by the harness instead.
fn bound_value_proven(widget: &SurfaceWidget, entry: &crate::surface::EvidenceEntry) -> bool {
    match widget {
        SurfaceWidget::MetricCard { value, .. } => value_present_in_evidence(entry, value),
        SurfaceWidget::SensorGauge { value, .. } => number_present_in_evidence(entry, *value),
        _ => true,
    }
}

fn layout_check(surface: &Surface) -> Result<(), ValidationError> {
    if surface.regions.is_empty() {
        return Err(err("layout", "surface has no regions"));
    }
    let widget_by_id: std::collections::HashMap<&str, &SurfaceWidget> = surface
        .widgets
        .iter()
        .map(|widget| (widget.id(), widget))
        .collect();
    let mut referenced: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for region in &surface.regions {
        for widget_id in &region.widgets {
            if widget_by_id.get(widget_id.as_str()).is_none() {
                return Err(err("layout", format!("region '{}' references unknown widget '{}'", region.id, widget_id)));
            }
            if !referenced.insert(widget_id.as_str()) {
                return Err(err("layout", format!("widget '{}' is referenced by more than one region", widget_id)));
            }
        }
    }
    if surface.layout.mode == LayoutMode::Grid {
        for region in &surface.regions {
            if region.widgets.is_empty() {
                return Err(err("layout", format!("grid region '{}' holds no widgets", region.id)));
            }
        }
    }
    for widget in &surface.widgets {
        if !referenced.contains(widget.id()) {
            return Err(err("layout", format!("widget '{}' is not referenced by any region", widget.id())));
        }
    }
    Ok(())
}

fn err(stage: &'static str, message: impl std::fmt::Display) -> ValidationError {
    ValidationError {
        stage,
        message: message.to_string(),
    }
}

/// Soft diagnostics the harness reports as warnings (never hard failures).
/// Returns one message per finding.
pub fn diagnostics(surface: &Surface, evidence: &EvidenceIndex) -> Vec<String> {
    let mut findings = Vec::new();
    for widget in &surface.widgets {
        let keys = widget.evidence();
        let any_key_matches = |check: &dyn Fn(&crate::surface::EvidenceEntry) -> bool| {
            keys.iter()
                .any(|key| evidence.get(key).map(check).unwrap_or(false))
        };
        match widget {
            SurfaceWidget::StatusList { items, .. } => {
                for item in items {
                    if let Some(detail) = &item.detail {
                        if !any_key_matches(&|entry| value_present_in_evidence(entry, detail)) {
                            findings.push(format!(
                                "widget '{}': status detail '{}' not found verbatim in evidence",
                                widget.id(),
                                detail
                            ));
                        }
                    }
                }
            }
            SurfaceWidget::Chart { data, .. } => {
                for point in data {
                    if !any_key_matches(&|entry| number_present_in_evidence(entry, point.value)) {
                        findings.push(format!(
                            "widget '{}': chart point {}={} not found as a standalone number in evidence",
                            widget.id(),
                            point.label,
                            point.value
                        ));
                    }
                }
            }
            SurfaceWidget::Notice { body, .. } => {
                if !any_key_matches(&|entry| value_present_in_evidence(entry, body)) {
                    findings.push(format!(
                        "widget '{}': notice body not found verbatim in evidence",
                        widget.id()
                    ));
                }
            }
            _ => {}
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::schema::{
        ChartPoint, DockEdge, LayoutMode, RegionPriority, StatusItem, SurfaceLayout,
        SurfacePlacement, SurfaceRegion,
    };
    use crate::tools::ToolResult;

    fn tool_result(tool: &'static str, text: &str) -> ToolResult {
        ToolResult {
            tool,
            text: text.to_string(),
        }
    }

    fn index_with(texts: &[&str]) -> EvidenceIndex {
        let results: Vec<ToolResult> = texts
            .iter()
            .enumerate()
            .map(|(i, t)| {
                tool_result(
                    if i == 0 {
                        "storage.observe_storage"
                    } else {
                        "power.observe_thermal"
                    },
                    t,
                )
            })
            .collect();
        EvidenceIndex::from_results(&results)
    }

    fn base_surface() -> Surface {
        Surface {
            intent: "how healthy is storage".to_string(),
            title: "Storage".to_string(),
            subtitle: None,
            placement: SurfacePlacement {
                edge: Some(DockEdge::Right),
                width: None,
                float: false,
            },
            layout: SurfaceLayout {
                mode: LayoutMode::Grid,
                columns: 12,
            },
            regions: vec![SurfaceRegion {
                id: "main".to_string(),
                span: 12,
                priority: RegionPriority::Primary,
                widgets: vec!["usage".to_string()],
            }],
            widgets: vec![SurfaceWidget::MetricCard {
                id: "usage".to_string(),
                title: "Disk used".to_string(),
                value: "81%".to_string(),
                unit: Some("%".to_string()),
                status: Some("healthy".to_string()),
                evidence: vec!["tool-0".to_string()],
            }],
        }
    }

    #[test]
    fn valid_surface_passes() {
        let surface = base_surface();
        let evidence = index_with(&["disk_used = 81%"]);
        assert!(
            validate(&surface, &evidence).is_ok(),
            "{:?}",
            validate(&surface, &evidence)
        );
    }

    #[test]
    fn zero_columns_rejected() {
        let mut surface = base_surface();
        surface.layout.columns = 0;
        let evidence = index_with(&["disk_used = 81%"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "schema");
        assert!(err.message.contains("columns"), "{err}");
    }

    #[test]
    fn region_span_overflows_columns() {
        let mut surface = base_surface();
        surface.layout.columns = 8;
        let evidence = index_with(&["disk_used = 81%"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "schema");
        assert!(err.message.contains("span 12"), "{err}");
    }

    #[test]
    fn missing_evidence_key_rejected() {
        let mut surface = base_surface();
        surface.widgets[0] = SurfaceWidget::MetricCard {
            id: "usage".to_string(),
            title: "Disk used".to_string(),
            value: "81%".to_string(),
            unit: Some("%".to_string()),
            status: None,
            evidence: vec!["tool-9".to_string()],
        };
        let evidence = index_with(&["disk_used = 81%"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "evidence");
        assert!(err.message.contains("tool-9"), "{err}");
    }

    #[test]
    fn widget_with_no_evidence_rejected() {
        let mut surface = base_surface();
        surface.widgets[0] = SurfaceWidget::Notice {
            id: "usage".to_string(),
            title: "Disk".to_string(),
            body: "disk_used = 81%".to_string(),
            evidence: vec![],
        };
        let evidence = index_with(&["disk_used = 81%"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "evidence");
        assert!(err.message.contains("binds no evidence"), "{err}");
    }

    #[test]
    fn fabricated_metric_value_rejected() {
        let mut surface = base_surface();
        surface.widgets[0] = SurfaceWidget::MetricCard {
            id: "usage".to_string(),
            title: "Disk used".to_string(),
            value: "97%".to_string(),
            unit: Some("%".to_string()),
            status: None,
            evidence: vec!["tool-0".to_string()],
        };
        let evidence = index_with(&["disk_used = 81%"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "evidence");
        assert!(err.message.contains("not present"), "{err}");
    }

    #[test]
    fn fabricated_gauge_value_rejected() {
        let mut surface = base_surface();
        surface.widgets[0] = SurfaceWidget::SensorGauge {
            id: "temp".to_string(),
            title: "Temp".to_string(),
            value: 99.0,
            min: Some(0.0),
            max: Some(100.0),
            unit: Some("C".to_string()),
            evidence: vec!["tool-0".to_string()],
        };
        surface.regions[0].widgets = vec!["temp".to_string()];
        let evidence = index_with(&["temperature = 63C"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "evidence");
        assert!(err.message.contains("not present"), "{err}");
    }

    #[test]
    fn cross_tool_evidence_does_not_satisfy_value() {
        let mut surface = base_surface();
        surface.widgets[0] = SurfaceWidget::SensorGauge {
            id: "temp".to_string(),
            title: "Temp".to_string(),
            value: 63.0,
            min: Some(0.0),
            max: Some(100.0),
            unit: Some("C".to_string()),
            evidence: vec!["tool-0".to_string()],
        };
        surface.regions[0].widgets = vec!["temp".to_string()];
        let evidence = index_with(&["disk_used = 81%", "temperature = 63C"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "evidence");
    }

    #[test]
    fn dangling_region_reference_rejected() {
        let mut surface = base_surface();
        surface.regions[0].widgets.push("ghost".to_string());
        let evidence = index_with(&["disk_used = 81%"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "layout");
        assert!(err.message.contains("ghost"), "{err}");
    }

    #[test]
    fn widget_referenced_twice_rejected() {
        let mut surface = base_surface();
        surface.regions.push(SurfaceRegion {
            id: "dup".to_string(),
            span: 4,
            priority: RegionPriority::Secondary,
            widgets: vec!["usage".to_string()],
        });
        let evidence = index_with(&["disk_used = 81%"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "layout");
        assert!(err.message.contains("more than one region"), "{err}");
    }

    #[test]
    fn unreferenced_widget_rejected() {
        let mut surface = base_surface();
        surface.widgets.push(SurfaceWidget::Chart {
            id: "history".to_string(),
            title: "History".to_string(),
            data: vec![ChartPoint {
                label: "now".to_string(),
                value: 12.0,
            }],
            evidence: vec!["tool-0".to_string()],
        });
        let evidence = index_with(&["disk_used = 81%"]);
        let err = validate(&surface, &evidence).unwrap_err();
        assert_eq!(err.stage, "layout");
        assert!(err.message.contains("not referenced"), "{err}");
    }

    #[test]
    fn status_list_detail_mismatch_is_diagnostic_not_error() {
        let mut surface = base_surface();
        surface.widgets.push(SurfaceWidget::StatusList {
            id: "checks".to_string(),
            title: "Checks".to_string(),
            items: vec![StatusItem {
                label: "SMART".to_string(),
                status: "healthy".to_string(),
                detail: Some("no faults".to_string()),
            }],
            evidence: vec!["tool-0".to_string()],
        });
        surface.regions.push(SurfaceRegion {
            id: "side".to_string(),
            span: 4,
            priority: RegionPriority::Secondary,
            widgets: vec!["checks".to_string()],
        });
        let evidence = index_with(&["disk_used = 81%"]);
        assert!(validate(&surface, &evidence).is_ok());
        let findings = diagnostics(&surface, &evidence);
        assert!(
            findings.iter().any(|f| f.contains("no faults")),
            "{findings:?}"
        );
    }

    #[test]
    fn chart_point_missing_is_diagnostic_not_error() {
        let mut surface = base_surface();
        surface.widgets.push(SurfaceWidget::Chart {
            id: "history".to_string(),
            title: "History".to_string(),
            data: vec![
                ChartPoint {
                    label: "1h".to_string(),
                    value: 12.0,
                },
                ChartPoint {
                    label: "2h".to_string(),
                    value: 41.0,
                },
            ],
            evidence: vec!["tool-0".to_string()],
        });
        surface.regions.push(SurfaceRegion {
            id: "side".to_string(),
            span: 4,
            priority: RegionPriority::Secondary,
            widgets: vec!["history".to_string()],
        });
        let evidence = index_with(&["disk_used = 81%, history 12 41"]);
        assert!(validate(&surface, &evidence).is_ok());
        let findings = diagnostics(&surface, &evidence);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn status_list_detail_present_passes_diagnostics() {
        let mut surface = base_surface();
        surface.widgets.push(SurfaceWidget::StatusList {
            id: "checks".to_string(),
            title: "Checks".to_string(),
            items: vec![StatusItem {
                label: "SMART".to_string(),
                status: "healthy".to_string(),
                detail: Some("disk_used = 81%".to_string()),
            }],
            evidence: vec!["tool-0".to_string()],
        });
        surface.regions.push(SurfaceRegion {
            id: "side".to_string(),
            span: 4,
            priority: RegionPriority::Secondary,
            widgets: vec!["checks".to_string()],
        });
        let evidence = index_with(&["disk_used = 81%"]);
        assert!(validate(&surface, &evidence).is_ok());
        let findings = diagnostics(&surface, &evidence);
        assert!(findings.is_empty(), "{findings:?}");
    }
}
