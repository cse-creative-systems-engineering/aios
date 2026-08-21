use super::*;

impl Coordinator {
    /// Compose a generative surface from the grounded answer and the tool
    /// evidence returned by a chat outcome. The composer is a separate model
    /// call with no tool access; a failure here is surfaced to the UI as a
    /// plain answer plus a `Notice` (Phase E), never as an empty panel.
    pub fn compose_surface(
        &self,
        intent: &str,
        answer: &str,
        evidence: &[crate::tools::ToolResult],
    ) -> Result<crate::surface::Surface, crate::surface::SurfaceComposeError> {
        self.compose_surface_with_meta(intent, answer, evidence)
            .map(|(surface, _)| surface)
    }

    /// Like `compose_surface`, but also returns the routing decision the
    /// gateway used for the composition call (which provider/model answered).
    pub fn compose_surface_with_meta(
        &self,
        intent: &str,
        answer: &str,
        evidence: &[crate::tools::ToolResult],
    ) -> Result<
        (crate::surface::Surface, crate::model::RoutingDecision),
        crate::surface::SurfaceComposeError,
    > {
        let index = crate::surface::EvidenceIndex::from_results(evidence);
        crate::surface::compose_surface_with_meta(
            &self.gateway,
            intent,
            answer,
            &index,
            self.compose_max_tokens,
        )
    }

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

