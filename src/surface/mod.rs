//! Groundless generative surfaces (ADR-0007).
//!
//! Aios does not design surfaces and keeps no widget vocabulary. It relays
//! the user request plus specialist evidence to a separate surface model
//! (`AgentRole::SurfaceComposition`), which returns a self-contained HTML
//! fragment. `composer` holds that relay call, the deterministic domain
//! coverage gate, and the value-fidelity gate; `evidence` holds the evidence
//! index both gates read from. Nothing here predetermines what a surface
//! looks like - the model draws it fresh from the data every time.

pub mod composer;
pub mod evidence;

pub use composer::{
    SurfaceComposeError, compose_unconstrained_html, coverage_gaps, verify_value_fidelity,
};
pub use evidence::{
    EvidenceEntry, EvidenceIndex, evidence_brief, number_present_in_evidence,
    value_present_in_evidence,
};
