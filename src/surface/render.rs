//! Headless surface rendering for the harness and later the frontend.
//!
//! `render_text` gives a terminal-readable tree; `render_html` emits a
//! self-contained preview page that maps the surface IR to layout semantics
//! (placement hints, grid columns, region spans) using the closed widget
//! vocabulary. This is a dev inspection tool - the Tauri canvas frontend
//! (Phase F) remains the real renderer - but it shares the same semantic
//! model, so the harness preview is a faithful stand-in for layout checks.

use crate::surface::schema::{
    DockEdge, LayoutMode, RegionPriority, Surface, SurfaceRegion, SurfaceWidget, WidthClass,
};

pub fn render_text(surface: &Surface) -> String {
    let mut lines = Vec::new();
    let title = match &surface.subtitle {
        Some(subtitle) => format!("{} - {}", surface.title, subtitle),
        None => surface.title.clone(),
    };
    lines.push(format!("Surface: {title}"));
    lines.push(format!(
        "  placement: {}",
        placement_text(&surface.placement)
    ));
    lines.push(format!(
        "  layout: {:?} columns={}",
        surface.layout.mode, surface.layout.columns
    ));
    let mode = match surface.layout.mode {
        LayoutMode::Grid => "grid",
        LayoutMode::Stack => "stack",
        LayoutMode::Row => "row",
    };
    lines.push(format!("  mode: {mode}"));
    for region in &surface.regions {
        lines.push(format!(
            "  region {} ({}, span {}):",
            region.id,
            priority_text(region.priority),
            region.span
        ));
        for widget_id in &region.widgets {
            if let Some(widget) = surface.widgets.iter().find(|w| w.id() == widget_id) {
                lines.push(format!("    {}", widget_text(widget)));
            }
        }
    }
    lines.join("\n")
}

fn placement_text(placement: &crate::surface::schema::SurfacePlacement) -> String {
    let mut parts = Vec::new();
    match placement.edge {
        Some(DockEdge::Left) => parts.push("dock left".to_string()),
        Some(DockEdge::Right) => parts.push("dock right".to_string()),
        Some(DockEdge::Top) => parts.push("dock top".to_string()),
        Some(DockEdge::Bottom) => parts.push("dock bottom".to_string()),
        None => parts.push("free".to_string()),
    }
    match placement.width {
        Some(WidthClass::Narrow) => parts.push("narrow".to_string()),
        Some(WidthClass::Medium) => parts.push("medium".to_string()),
        Some(WidthClass::Wide) => parts.push("wide".to_string()),
        None => {}
    }
    if placement.float {
        parts.push("float".to_string());
    }
    parts.join(", ")
}

fn priority_text(priority: RegionPriority) -> &'static str {
    match priority {
        RegionPriority::Primary => "primary",
        RegionPriority::Secondary => "secondary",
        RegionPriority::Tertiary => "tertiary",
    }
}

fn widget_text(widget: &SurfaceWidget) -> String {
    match widget {
        SurfaceWidget::MetricCard {
            id,
            title,
            value,
            unit,
            status,
            evidence,
        } => format!(
            "[metricCard] {}: {}{}{}  <- {}  (evidence: {})",
            title,
            value,
            unit.as_deref().unwrap_or(""),
            status
                .as_deref()
                .map(|s| format!(" ({s})"))
                .unwrap_or_default(),
            id,
            evidence.join(", ")
        ),
        SurfaceWidget::SensorGauge {
            id,
            title,
            value,
            min,
            max,
            unit,
            evidence,
        } => format!(
            "[sensorGauge] {}: {}{} ({}{})  <- {}  (evidence: {})",
            title,
            value,
            unit.as_deref().unwrap_or(""),
            min.map(|m| format!("min {m}")).unwrap_or_default(),
            max.map(|m| format!(" max {m}")).unwrap_or_default(),
            id,
            evidence.join(", ")
        ),
        SurfaceWidget::StatusList {
            id,
            title,
            items,
            evidence,
        } => {
            let rows: Vec<String> = items
                .iter()
                .map(|item| {
                    let detail = item
                        .detail
                        .as_deref()
                        .map(|d| format!(": {d}"))
                        .unwrap_or_default();
                    format!("{}=>{}{}", item.label, item.status, detail)
                })
                .collect();
            format!(
                "[statusList] {}: {}  <- {}  (evidence: {})",
                title,
                rows.join(" | "),
                id,
                evidence.join(", ")
            )
        }
        SurfaceWidget::Chart {
            id,
            title,
            data,
            evidence,
        } => {
            let points: Vec<String> = data
                .iter()
                .map(|point| format!("{}={}", point.label, point.value))
                .collect();
            format!(
                "[chart] {}: {}  <- {}  (evidence: {})",
                title,
                points.join(", "),
                id,
                evidence.join(", ")
            )
        }
        SurfaceWidget::Notice {
            id,
            title,
            body,
            evidence,
        } => format!(
            "[notice] {}: {}  <- {}  (evidence: {})",
            title,
            body,
            id,
            evidence.join(", ")
        ),
    }
}

/// Self-contained HTML preview. Pure layout semantics: placement hints become
/// panel classes, grid columns become CSS columns, region spans become column
/// spans. No external assets, no scripts.
pub fn render_html(surface: &Surface) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>",
        html_escape(&surface.title)
    ));
    body.push_str(STYLE);
    body.push_str("</style></head><body>");
    body.push_str(&format!(
        "<h1>{}</h1>",
        html_escape(&surface.title)
    ));
    if let Some(subtitle) = &surface.subtitle {
        body.push_str(&format!(
            "<p class=\"subtitle\">{}</p>",
            html_escape(subtitle)
        ));
    }
    body.push_str(&format!(
        "<p class=\"meta\">intent: {} | {} | version 1</p>",
        html_escape(&surface.intent),
        placement_text(&surface.placement)
    ));
    body.push_str("<div class=\"panel\">");
    body.push_str(&format!(
        "<div class=\"grid cols-{}\">",
        surface.layout.columns
    ));
    for region in &surface.regions {
        body.push_str(&region_html(surface, region));
    }
    body.push_str("</div></div>");
    body.push_str("</body></html>");
    body
}

fn region_html(surface: &Surface, region: &SurfaceRegion) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<section class=\"region {} span-{}\"><h2>{}</h2>",
        match region.priority {
            RegionPriority::Primary => "primary",
            RegionPriority::Secondary => "secondary",
            RegionPriority::Tertiary => "tertiary",
        },
        region.span,
        html_escape(&region.id)
    ));
    for widget_id in &region.widgets {
        if let Some(widget) = surface.widgets.iter().find(|w| w.id() == widget_id) {
            out.push_str(&widget_html(widget));
        }
    }
    out.push_str("</section>");
    out
}

fn widget_html(widget: &SurfaceWidget) -> String {
    let evidence = widget.evidence();
    let evidence_html = evidence_chips(evidence);
    let inner = match widget {
        SurfaceWidget::MetricCard {
            value, unit, status, ..
        } => {
            let status_html = status
                .as_deref()
                .map(|s| format!("<span class=\"chip {}\">{}</span>", status_class(s), html_escape(s)))
                .unwrap_or_default();
            format!(
                "<div class=\"metric\"><span class=\"value\">{}{}</span>{}</div>",
                html_escape(value),
                unit.as_deref().map(|u| format!(" <span class=\"unit\">{}</span>", html_escape(u))).unwrap_or_default(),
                status_html
            )
        }
        SurfaceWidget::SensorGauge {
            value, min, max, unit, ..
        } => {
            let (lo, hi) = (min.unwrap_or(0.0), max.unwrap_or(100.0));
            let pct = if hi > lo {
                ((value - lo) / (hi - lo) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            format!(
                "<div class=\"gauge\"><span class=\"value\">{}{}</span><div class=\"bar\"><div class=\"fill\" style=\"width:{}%\"></div></div><span class=\"range\">{}-{}</span></div>",
                value,
                unit.as_deref().map(|u| format!(" {}", html_escape(u))).unwrap_or_default(),
                pct,
                lo,
                hi
            )
        }
        SurfaceWidget::StatusList { items, .. } => {
            let rows: String = items
                .iter()
                .map(|item| {
                    let detail = item
                        .detail
                        .as_deref()
                        .map(|d| format!("<span class=\"detail\">{}</span>", html_escape(d)))
                        .unwrap_or_default();
                    format!(
                        "<li><span class=\"dot {}\"></span><span class=\"label\">{}</span><span class=\"status\">{}</span>{}</li>",
                        status_class(&item.status),
                        html_escape(&item.label),
                        html_escape(&item.status),
                        detail
                    )
                })
                .collect();
            format!("<ul class=\"status\">{rows}</ul>")
        }
        SurfaceWidget::Chart { data, .. } => {
            let max = data.iter().map(|p| p.value).fold(0.0, f64::max).max(1.0);
            let bars: String = data
                .iter()
                .map(|point| {
                    let h = ((point.value / max) * 100.0).max(2.0);
                    format!(
                        "<div class=\"bar\"><div class=\"col\" style=\"height:{}%\"></div><span class=\"barlabel\">{}</span></div>",
                        h,
                        html_escape(&point.label)
                    )
                })
                .collect();
            format!(
                "<div class=\"chart\"><div class=\"bars\">{bars}</div></div>"
            )
        }
        SurfaceWidget::Notice { body, .. } => {
            format!("<div class=\"notice\">{}</div>", html_escape(body))
        }
    };
    format!(
        "<article class=\"widget\"><h3>{}</h3>{}{}</article>",
        html_escape(widget_title(widget)),
        inner,
        evidence_html
    )
}

fn widget_title(widget: &SurfaceWidget) -> &str {
    match widget {
        SurfaceWidget::MetricCard { title, .. }
        | SurfaceWidget::SensorGauge { title, .. }
        | SurfaceWidget::StatusList { title, .. }
        | SurfaceWidget::Chart { title, .. }
        | SurfaceWidget::Notice { title, .. } => title,
    }
}

fn evidence_chips(keys: &[String]) -> String {
    let chips: String = keys
        .iter()
        .map(|key| format!("<span class=\"evidence\">{}</span>", html_escape(key)))
        .collect();
    format!("<div class=\"evidence-row\">{chips}</div>")
}

fn status_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "healthy" | "ok" | "up" | "pass" => "good",
        "warn" | "warning" | "degraded" => "warn",
        "fail" | "down" | "error" => "bad",
        _ => "neutral",
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = r#"
body { font-family: system-ui, sans-serif; background: #101418; color: #e6e8ea; margin: 24px; }
h1 { margin-bottom: 4px; }
.subtitle { color: #9aa4ad; margin: 0 0 8px; }
.meta { color: #6b7680; font-size: 12px; }
.panel { background: #171c22; border: 1px solid #262d35; border-radius: 10px; padding: 16px; }
.grid { display: grid; gap: 12px; }
.cols-8 { grid-template-columns: repeat(8, 1fr); }
.cols-12 { grid-template-columns: repeat(12, 1fr); }
.region { background: #1d242c; border: 1px solid #2a323c; border-radius: 8px; padding: 12px; }
.region.primary { border-left: 3px solid #4a9eff; }
.region.secondary { border-left: 3px solid #7fd15c; }
.region.tertiary { border-left: 3px solid #b58dff; }
.region h2 { margin: 0 0 8px; font-size: 12px; text-transform: uppercase; letter-spacing: .08em; color: #9aa4ad; }
.region.span-1 { grid-column: span 1; } .region.span-2 { grid-column: span 2; }
.region.span-3 { grid-column: span 3; } .region.span-4 { grid-column: span 4; }
.region.span-5 { grid-column: span 5; } .region.span-6 { grid-column: span 6; }
.region.span-7 { grid-column: span 7; } .region.span-8 { grid-column: span 8; }
.region.span-9 { grid-column: span 9; } .region.span-10 { grid-column: span 10; }
.region.span-11 { grid-column: span 11; } .region.span-12 { grid-column: span 12; }
.widget { background: #11161b; border: 1px solid #232b34; border-radius: 6px; padding: 10px; }
.widget h3 { margin: 0 0 8px; font-size: 13px; }
.metric .value { font-size: 26px; font-weight: 600; }
.unit { color: #9aa4ad; font-size: 14px; }
.chip { display: inline-block; margin-left: 8px; padding: 2px 8px; border-radius: 10px; font-size: 11px; }
.chip.good { background: #16301c; color: #7fd15c; }
.chip.warn { background: #33280f; color: #e6b84a; }
.chip.bad { background: #3a1717; color: #ff7a7a; }
.chip.neutral { background: #232b34; color: #9aa4ad; }
.gauge .value { font-size: 22px; font-weight: 600; }
.bar { background: #232b34; border-radius: 4px; height: 8px; margin: 8px 0; overflow: hidden; }
.bar .fill { background: #4a9eff; height: 100%; }
.range { color: #6b7680; font-size: 11px; }
ul.status { list-style: none; margin: 0; padding: 0; }
ul.status li { display: flex; align-items: center; gap: 8px; padding: 4px 0; border-bottom: 1px solid #1c232b; }
.dot { width: 8px; height: 8px; border-radius: 50%; }
.dot.good { background: #7fd15c; } .dot.warn { background: #e6b84a; } .dot.bad { background: #ff7a7a; } .dot.neutral { background: #6b7680; }
.label { color: #c8cdd2; } .status { margin-left: auto; } .detail { color: #9aa4ad; font-size: 12px; }
.chart .bars { display: flex; align-items: flex-end; gap: 8px; height: 90px; }
.chart .bar { flex: 1; display: flex; flex-direction: column; justify-content: flex-end; align-items: center; background: none; height: 100%; }
.chart .col { background: #4a9eff; width: 100%; border-radius: 3px 3px 0 0; }
.barlabel { font-size: 10px; color: #9aa4ad; margin-top: 4px; }
.notice { background: #2a2410; border: 1px solid #4d4215; border-radius: 6px; padding: 8px; color: #e6c95a; }
.evidence-row { margin-top: 8px; }
.evidence { display: inline-block; background: #20272f; border: 1px solid #2c3440; border-radius: 4px; padding: 1px 6px; margin-right: 4px; font-size: 10px; color: #6b7680; }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::schema::{
        ChartPoint, DockEdge, LayoutMode, RegionPriority, StatusItem, SurfaceLayout,
        SurfacePlacement, SurfaceRegion,
    };

    fn sample() -> Surface {
        Surface {
            intent: "how is the disk".to_string(),
            title: "Disk <status>".to_string(),
            subtitle: Some("live".to_string()),
            placement: SurfacePlacement {
                edge: Some(DockEdge::Right),
                width: Some(WidthClass::Narrow),
                float: false,
            },
            layout: SurfaceLayout {
                mode: LayoutMode::Grid,
                columns: 12,
            },
            regions: vec![
                SurfaceRegion {
                    id: "main".to_string(),
                    span: 8,
                    priority: RegionPriority::Primary,
                    widgets: vec!["usage".to_string()],
                },
                SurfaceRegion {
                    id: "side".to_string(),
                    span: 4,
                    priority: RegionPriority::Secondary,
                    widgets: vec!["temp".to_string(), "checks".to_string()],
                },
            ],
            widgets: vec![
                SurfaceWidget::MetricCard {
                    id: "usage".to_string(),
                    title: "Used".to_string(),
                    value: "81%".to_string(),
                    unit: Some("%".to_string()),
                    status: Some("healthy".to_string()),
                    evidence: vec!["tool-0".to_string()],
                },
                SurfaceWidget::SensorGauge {
                    id: "temp".to_string(),
                    title: "Temp".to_string(),
                    value: 63.0,
                    min: Some(0.0),
                    max: Some(100.0),
                    unit: Some("C".to_string()),
                    evidence: vec!["tool-1".to_string()],
                },
                SurfaceWidget::StatusList {
                    id: "checks".to_string(),
                    title: "Checks".to_string(),
                    items: vec![StatusItem {
                        label: "SMART".to_string(),
                        status: "ok".to_string(),
                        detail: Some("no faults".to_string()),
                    }],
                    evidence: vec!["tool-0".to_string()],
                },
            ],
        }
    }

    #[test]
    fn text_render_shows_regions_and_widgets() {
        let text = render_text(&sample());
        assert!(text.contains("region main (primary, span 8)"), "{text}");
        assert!(text.contains("[metricCard] Used: 81%%"), "{text}");
        assert!(text.contains("[sensorGauge] Temp: 63C"), "{text}");
        assert!(text.contains("[statusList] Checks"), "{text}");
        assert!(text.contains("dock right, narrow"), "{text}");
    }

    #[test]
    fn html_render_escapes_content() {
        let html = render_html(&sample());
        assert!(html.contains("Disk &lt;status&gt;"), "{html}");
        assert!(html.contains("span-8"), "{html}");
        assert!(html.contains("cols-12"), "{html}");
        assert!(html.contains("class=\"metric\""), "{html}");
        assert!(html.contains("tool-0"), "{html}");
    }

    #[test]
    fn html_escapes_quotes_and_ampersands() {
        assert_eq!(html_escape("a&\"b"), "a&amp;&quot;b");
    }

    #[test]
    fn chart_renders_bars() {
        let mut surface = sample();
        surface.widgets.push(SurfaceWidget::Chart {
            id: "hist".to_string(),
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
            id: "hist-region".to_string(),
            span: 4,
            priority: RegionPriority::Tertiary,
            widgets: vec!["hist".to_string()],
        });
        let html = render_html(&surface);
        assert!(html.contains("class=\"chart\""), "{html}");
        assert!(html.contains("1h"), "{html}");
    }
}
