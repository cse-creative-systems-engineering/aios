//! Generative surface intermediate representation (surface/v1).
//!
//! `Surface` is the typed contract between the AI composition layer and the
//! renderer: the model describes the surface, it never renders it. The closed
//! widget vocabulary and layout grammar live in `schema`; later phases add the
//! evidence index and value checks (`evidence`), the model-driven composer
//! call (`composer`), and deterministic schema/evidence/layout validation
//! (`validator`).

pub mod schema;

pub use schema::{
    ChartPoint, DockEdge, LayoutMode, RegionPriority, StatusItem, Surface, SurfaceLayout,
    SurfacePlacement, SurfaceRegion, SurfaceWidget, WidthClass,
};

/// Current surface contract version (`surface/v1`).
pub const SURFACE_VERSION: u32 = schema::SURFACE_VERSION;
