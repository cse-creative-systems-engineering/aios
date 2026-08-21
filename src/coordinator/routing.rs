use super::*;

impl Coordinator {
    /// The assignment the surface composer uses (role id `surface`), if one
    /// is configured. Named `current_route` for the sidebar status row that
    /// shows the generative-surface model.
    pub fn current_route(&self) -> Result<RoutingDecision, RoutingError> {
        self.role_assignment_decision("surface")
    }

    pub fn chat_route(&self) -> Result<RoutingDecision, RoutingError> {
        self.role_assignment_decision("chat")
    }

    fn role_assignment_decision(&self, role: &str) -> Result<RoutingDecision, RoutingError> {
        let (provider, model) = self
            .gateway
            .router()
            .assignment(role)
            .ok_or_else(|| RoutingError::NoAssignmentForRole(role.to_string()))?;
        Ok(RoutingDecision {
            provider,
            model,
            connectivity_state: self.gateway.router().connectivity(),
            data_classification: DataClassification::Public,
            reduced_confidence: false,
        })
    }

    // ---- Settings panel: provider and role administration ----
    //
    // The panel edits typed settings through these methods; it never routes
    // model requests or holds provider credentials (docs/ui.md). API keys
    // are write-only from the frontend's point of view: they go in, are
    // persisted, and are never returned.

    /// Assign a provider/model pair to a role. The pair is validated against
    /// the provider's live discovery catalogue before acceptance; an unknown
    /// model or a provider whose discovery failed is rejected with the reason.
    pub fn set_role_assignment(
        &mut self,
        role: &str,
        provider_id: &str,
        model: &str,
    ) -> Result<(), String> {
        self.validated_catalogue_model(provider_id, model)?;
        self.store_assignment(role, provider_id, model)?;
        self.persist_config()
    }

    /// Assign one provider/model pair to every role in a group. The pair is
    /// validated once, then applied to each role; config is persisted once at
    /// the end. Returns the role ids that were assigned.
    ///
    /// Groups:
    /// - `all`: chat, verification, surface, and every specialist
    /// - `specialists`: the eleven specialist roles only
    pub fn set_role_group_assignment(
        &mut self,
        group: &str,
        provider_id: &str,
        model: &str,
    ) -> Result<Vec<String>, String> {
        let roles: Vec<String> = match group {
            "all" => assignable_roles().into_iter().map(|r| r.id).collect(),
            "specialists" => SPECIALIST_DOMAINS
                .iter()
                .map(|d| format!("specialist:{d}"))
                .collect(),
            other => return Err(format!("unknown role group '{other}'")),
        };
        self.validated_catalogue_model(provider_id, model)?;
        for role in &roles {
            self.store_assignment(role, provider_id, model)?;
        }
        self.persist_config()?;
        Ok(roles)
    }

    /// Check that the provider is configured and its discovery catalogue
    /// lists the model. Refreshes the catalogue once if it is missing or the
    /// last discovery failed.
    fn validated_catalogue_model(&mut self, provider_id: &str, model: &str) -> Result<(), String> {
        if self.config.provider(provider_id).is_none() {
            return Err(format!("provider '{provider_id}' is not configured"));
        }
        let cached = self
            .catalogue
            .read()
            .expect("catalogue lock")
            .get(provider_id)
            .cloned();
        let catalogue = match cached {
            Some(entry) if entry.error.is_none() => entry,
            _ => {
                // No successful discovery yet (or it failed earlier): try once
                // now so a stale failure does not block a valid assignment.
                let _ = self.refresh_catalogue(provider_id);
                self.catalogue
                    .read()
                    .expect("catalogue lock")
                    .get(provider_id)
                    .cloned()
                    .ok_or_else(|| format!("no models were found for provider '{provider_id}'"))?
            }
        };
        if let Some(error) = catalogue.error {
            return Err(error);
        }
        if !catalogue.models.iter().any(|m| m.id == model) {
            return Err(format!(
                "model '{model}' was not found for provider '{provider_id}'"
            ));
        }
        Ok(())
    }

    /// Apply an already-validated assignment to the live router and the
    /// config roles section. Does not persist; callers batch or persist.
    fn store_assignment(&mut self, role: &str, provider_id: &str, model: &str) -> Result<(), String> {
        validate_role_id(role)?;
        self.gateway
            .router()
            .set_assignment(role, ProviderId::new(provider_id), ModelId::new(model))
            .map_err(|e| e.to_string())?;

        let roles = self.config.roles.get_or_insert_with(Default::default);
        let assignment = crate::config::RoleAssignment {
            provider: provider_id.to_string(),
            model: model.to_string(),
        };
        match role {
            "surface" => roles.surface = Some(assignment),
            "chat" => roles.chat = Some(assignment),
            "verification" => roles.verification = Some(assignment),
            specialist => {
                if let Some(domain) = specialist.strip_prefix("specialist:") {
                    roles.specialists.insert(domain.to_string(), assignment);
                }
            }
        }
        Ok(())
    }

    /// The resolved assignment for a role, or None when the role has no
    /// assignment yet. There is no fallback: an unassigned role simply has
    /// no route until the user picks one in the settings panel.
    pub fn role_route(&self, role: &str) -> Result<Option<RoutingDecision>, String> {
        validate_role_id(role)?;
        Ok(self.role_assignment_decision(role).ok())
    }
}

/// A role the settings panel can assign a provider/model to.
pub struct RoleDescriptor {
    pub id: String,
    pub label: String,
    pub detail: String,
    /// What model strengths make a good fit for this role, shown in the
    /// settings panel to guide assignment.
    pub fit: String,
}

/// The specialist domains that get their own assignable role. Each maps to
/// the role id `specialist:<domain>`.
pub const SPECIALIST_DOMAINS: &[&str] = &[
    "wifi", "storage", "network", "drivers", "graphics", "memory", "power", "processes",
    "security", "boot", "packages",
];

/// Every assignable role, in panel order: the three agent roles first, then
/// one row per specialist domain.
pub fn assignable_roles() -> Vec<RoleDescriptor> {
    let mut roles = vec![
        RoleDescriptor {
            id: "chat".into(),
            label: "Chat".into(),
            detail: "The planner model behind the chat interface.".into(),
            fit: "Needs reliable tool calling and long-context reasoning; the strongest model you have.".into(),
        },
        RoleDescriptor {
            id: "verification".into(),
            label: "Verification".into(),
            detail: "Reviews plans before anything is executed.".into(),
            fit: "Needs careful, conservative reasoning that sticks to evidence; accuracy over speed.".into(),
        },
        RoleDescriptor {
            id: "surface".into(),
            label: "Surface".into(),
            detail: "Composes generative canvas widgets from answers.".into(),
            fit: "Needs dependable structured output and exact instruction following.".into(),
        },
    ];
    for domain in SPECIALIST_DOMAINS {
        roles.push(RoleDescriptor {
            id: format!("specialist:{domain}"),
            label: format!("{domain} specialist"),
            detail: format!("Answers {domain} questions from live system evidence."),
            fit: "Short grounded summaries over tool evidence; fast, inexpensive models work well.".into(),
        });
    }
    roles
}

/// Check that a role id is one the panel can assign to.
fn validate_role_id(role: &str) -> Result<(), String> {
    if matches!(role, "chat" | "verification" | "surface") {
        return Ok(());
    }
    if let Some(domain) = role.strip_prefix("specialist:") {
        if SPECIALIST_DOMAINS.contains(&domain) {
            return Ok(());
        }
    }
    Err(format!("unknown role '{role}'"))
}
