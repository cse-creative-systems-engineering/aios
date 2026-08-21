use super::*;

impl Coordinator {
    /// Relay the user intent and specialist evidence to the groundless
    /// surface model (ADR-0007). Aios gathers and verifies; the model alone
    /// authors the presentation. A failure here is surfaced to the UI as a
    /// plain answer, never as a broken panel.
    pub fn compose_unconstrained_html(
        &self,
        intent: &str,
        evidence: &[crate::tools::ToolResult],
        previous_html: Option<&str>,
    ) -> Result<(String, crate::model::RoutingDecision), crate::surface::SurfaceComposeError> {
        let index = crate::surface::EvidenceIndex::from_results(evidence);
        crate::surface::compose_unconstrained_html(
            &self.gateway,
            intent,
            &index,
            previous_html,
            self.compose_max_tokens,
        )
    }
}
