use crate::capability::Operation;
use crate::protocol::{
    GuardianDecision, GuardianVerdict, InvariantId, ToolRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvariantSeverity {
    Safety,
    Boot,
    Availability,
    Performance,
    Experience,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvariantCheck {
    AlwaysAllow,
    BlockOperation(Operation),
    BlockFirmwareWriteUnlessTested,
    BlockKernelModuleUnlessTested,
    BlockBootConfigUnlessFallbackExists,
}

#[derive(Clone, Debug)]
pub struct InvariantRule {
    pub id: InvariantId,
    pub description: String,
    pub severity: InvariantSeverity,
    pub check: InvariantCheck,
}

#[derive(Clone, Debug, Default)]
pub struct Guardian {
    invariants: Vec<InvariantRule>,
    tested_firmware: HashSet<String>,
    tested_drivers: HashSet<String>,
    fallback_images: HashSet<String>,
}

impl Guardian {
    pub fn new() -> Self {
        Self {
            invariants: vec![
                InvariantRule {
                    id: "FIRMWARE-001".into(),
                    description: "untested firmware cannot be activated".into(),
                    severity: InvariantSeverity::Safety,
                    check: InvariantCheck::BlockFirmwareWriteUnlessTested,
                },
                InvariantRule {
                    id: "DRIVER-001".into(),
                    description: "an untested driver cannot be activated in the current boot environment".into(),
                    severity: InvariantSeverity::Boot,
                    check: InvariantCheck::BlockKernelModuleUnlessTested,
                },
                InvariantRule {
                    id: "BOOT-001".into(),
                    description: "boot configuration changes require a tested fallback image".into(),
                    severity: InvariantSeverity::Boot,
                    check: InvariantCheck::BlockBootConfigUnlessFallbackExists,
                },
            ],
            tested_firmware: HashSet::new(),
            tested_drivers: HashSet::new(),
            fallback_images: HashSet::new(),
        }
    }

    pub fn add_rule(&mut self, rule: InvariantRule) {
        self.invariants.push(rule);
    }

    pub fn mark_firmware_tested(&mut self, firmware_ref: impl Into<String>) {
        self.tested_firmware.insert(firmware_ref.into());
    }

    pub fn mark_driver_tested(&mut self, module: impl Into<String>) {
        self.tested_drivers.insert(module.into());
    }

    pub fn add_fallback_image(&mut self, image: impl Into<String>) {
        self.fallback_images.insert(image.into());
    }

    pub fn invariants(&self) -> &[InvariantRule] {
        &self.invariants
    }

    fn check_rule(&self, rule: &InvariantRule, request: &ToolRequest) -> Option<String> {
        let why_blocked = match &rule.check {
            InvariantCheck::AlwaysAllow => return None,
            InvariantCheck::BlockOperation(op) if *op == request.operation => {
                Some(format!("operation {:?} is blocked by {}",
                    request.operation, rule.id))
            }
            InvariantCheck::BlockOperation(_) => return None,
            InvariantCheck::BlockFirmwareWriteUnlessTested => {
                if request.operation != Operation::FirmwareWrite {
                    return None;
                }
                match &request.parameters {
                    crate::protocol::ToolParameters::FirmwareWrite { firmware_ref } => {
                        if self.tested_firmware.contains(firmware_ref) {
                            return None;
                        }
                        Some(format!(
                            "{}: untested firmware cannot be activated (ref: {firmware_ref})",
                            rule.id
                        ))
                    }
                    _ => Some(format!(
                        "{}: firmware write requires a firmware reference",
                        rule.id
                    )),
                }
            }
            InvariantCheck::BlockKernelModuleUnlessTested => {
                if request.operation != Operation::KernelModule {
                    return None;
                }
                match &request.parameters {
                    crate::protocol::ToolParameters::KernelModule { action, module } => {
                        if action == "unload" || self.tested_drivers.contains(module) {
                            return None;
                        }
                        Some(format!(
                            "{}: untested driver cannot be activated (module: {module})",
                            rule.id
                        ))
                    }
                    _ => Some(format!("{}: kernel module action requires a module", rule.id)),
                }
            }
            InvariantCheck::BlockBootConfigUnlessFallbackExists => {
                if request.operation != Operation::BootConfig {
                    return None;
                }
                if self.fallback_images.is_empty() {
                    return Some(format!(
                        "{}: no tested fallback image exists for boot configuration change",
                        rule.id
                    ));
                }
                None
            }
        };
        why_blocked
    }

    pub fn review(&self, request: &ToolRequest) -> GuardianDecision {
        let mut rule_references = Vec::new();
        let mut explanation_parts = Vec::new();
        for rule in &self.invariants {
            if let Some(reason) = self.check_rule(rule, request) {
                rule_references.push(rule.id.clone());
                explanation_parts.push(reason);
            }
        }
        let decision = if explanation_parts.is_empty() {
            GuardianVerdict::Allow
        } else {
            GuardianVerdict::Block(explanation_parts.join("; "))
        };
        let explanation = match &decision {
            GuardianVerdict::Allow => "no invariant violated".into(),
            GuardianVerdict::Block(reason) => reason.clone(),
        };
        GuardianDecision {
            envelope: crate::protocol::MessageEnvelope::new(
                crate::protocol::MessageType::GuardianDecision,
                crate::capability::PrincipalId::system("guardian"),
                request.envelope.correlation_id,
                crate::protocol::DataClassification::Protected,
            ),
            request_id: request.request_id,
            decision,
            affected_systems: vec![request.resource.clone()],
            rule_references,
            explanation,
        }
    }
}

impl crate::capability::GuardianClient for Guardian {
    fn review(&self, request: &ToolRequest) -> GuardianVerdict {
        self.review(request).decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{DataClassification, MessageType};

    fn request(
        resource: &str,
        operation: Operation,
        parameters: crate::protocol::ToolParameters,
    ) -> ToolRequest {
        let principal = crate::capability::PrincipalId::agent("wifi.specialist", "wifi0");
        let token = crate::capability::CapabilityToken {
            principal: principal.clone(),
            capability: crate::capability::Capability {
                resource: crate::capability::ResourceId(resource.into()),
                operation,
            },
            clearance: crate::capability::Clearance::max(),
            granted_at: 0,
            expires_at: 999999,
            provenance: crate::capability::Provenance {
                granted_by: crate::capability::PrincipalId::user(),
                package_id: "wifi.specialist".into(),
                package_version: 1,
                signature_verified: true,
            },
        };
        ToolRequest {
            envelope: crate::protocol::MessageEnvelope::new(
                MessageType::ToolRequest,
                principal.clone(),
                uuid::Uuid::new_v4(),
                DataClassification::SystemConfig,
            ),
            request_id: uuid::Uuid::new_v4(),
            principal,
            resource: crate::capability::ResourceId(resource.into()),
            operation,
            tool_id: "wifi.firmware_write".into(),
            capability_token: token,
            parameters,
            plan_hash: None,
            action_id: None,
            nonce: 1,
        }
    }

    #[test]
    fn guardian_allows_read_only_operations() {
        let g = Guardian::new();
        let req = request(
            "device:wifi0",
            Operation::Observe,
            crate::protocol::ToolParameters::Observe { fields: vec![] },
        );
        assert_eq!(g.review(&req).decision, GuardianVerdict::Allow);
    }

    #[test]
    fn guardian_blocks_untested_firmware() {
        let g = Guardian::new();
        let req = request(
            "device:wifi0",
            Operation::FirmwareWrite,
            crate::protocol::ToolParameters::FirmwareWrite {
                firmware_ref: "iwlwifi-ucode-72".into(),
            },
        );
        let decision = g.review(&req);
        assert!(matches!(decision.decision, GuardianVerdict::Block(_)));
        assert!(decision.rule_references.contains(&"FIRMWARE-001".into()));
    }

    #[test]
    fn guardian_allows_tested_firmware() {
        let mut g = Guardian::new();
        g.mark_firmware_tested("iwlwifi-ucode-72");
        let req = request(
            "device:wifi0",
            Operation::FirmwareWrite,
            crate::protocol::ToolParameters::FirmwareWrite {
                firmware_ref: "iwlwifi-ucode-72".into(),
            },
        );
        assert_eq!(g.review(&req).decision, GuardianVerdict::Allow);
    }

    #[test]
    fn guardian_blocks_untested_driver_load() {
        let g = Guardian::new();
        let req = request(
            "device:wifi0",
            Operation::KernelModule,
            crate::protocol::ToolParameters::KernelModule {
                action: "load".into(),
                module: "iwlwifi-next".into(),
            },
        );
        let decision = g.review(&req);
        assert!(matches!(decision.decision, GuardianVerdict::Block(_)));
        assert!(decision.rule_references.contains(&"DRIVER-001".into()));
    }

    #[test]
    fn guardian_blocks_boot_config_without_fallback() {
        let g = Guardian::new();
        let req = request(
            "boot:systemd-boot",
            Operation::BootConfig,
            crate::protocol::ToolParameters::BootConfig {
                changes: serde_json::json!({}),
            },
        );
        let decision = g.review(&req);
        assert!(matches!(decision.decision, GuardianVerdict::Block(_)));
        assert!(decision.rule_references.contains(&"BOOT-001".into()));
    }

    #[test]
    fn guardian_allows_boot_config_with_fallback_image() {
        let mut g = Guardian::new();
        g.add_fallback_image("known-good-2026-08-01");
        let req = request(
            "boot:systemd-boot",
            Operation::BootConfig,
            crate::protocol::ToolParameters::BootConfig {
                changes: serde_json::json!({}),
            },
        );
        assert_eq!(g.review(&req).decision, GuardianVerdict::Allow);
    }
}
