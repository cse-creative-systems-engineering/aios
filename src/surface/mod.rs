//! Generative surface intermediate representation (surface/v1).
//!
//! `Surface` is the typed contract between the AI composition layer and the
//! renderer: the model describes the surface, it never renders it. The closed
//! widget vocabulary and layout grammar live in `schema`, the evidence index
//! and value checks in `evidence`, the model-driven composer call in
//! `composer`, deterministic schema/evidence/layout validation in `validator`,
//! and headless text/HTML previews in `render`.

pub mod composer;
pub mod evidence;
pub mod render;
pub mod schema;
pub mod stub;
pub mod validator;

pub use composer::{
    SurfaceComposeError, compose_surface, compose_surface_with_meta, compose_unconstrained_html,
    coverage_gaps, surface_composition_instructions, verify_value_fidelity,
};
pub use evidence::{
    EvidenceEntry, EvidenceIndex, evidence_brief, number_present_in_evidence,
    value_present_in_evidence,
};
pub use render::{render_html, render_text};
pub use schema::{
    ChartPoint, DockEdge, LayoutMode, RegionPriority, StatusItem, Surface, SurfaceDensity,
    SurfaceLayout, SurfacePlacement, SurfaceRegion, SurfaceWidget, WidthClass,
};
pub use validator::{ValidationError, diagnostics, validate, validate_for_intent};

/// Current surface contract version (`surface/v1`).
pub const SURFACE_VERSION: u32 = schema::SURFACE_VERSION;
