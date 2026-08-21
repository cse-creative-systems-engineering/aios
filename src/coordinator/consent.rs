use super::*;

impl Coordinator {
    pub fn grant_consent(&self, provider: &str, class: DataClassification) -> Result<(), String> {
        let provider_id = ProviderId::new(provider);
        let record = crate::model::ConsentRecord::new(provider_id.clone(), vec![class]);
        let result = self
            .gateway
            .router()
            .grant_consent(record)
            .map_err(|e| e.to_string());
        match &result {
            Ok(()) => self.record_audit(
                "user",
                "consent",
                &format!("{provider} {class:?}"),
                "granted",
            ),
            Err(e) => self.record_audit(
                "user",
                "consent",
                &format!("{provider} {class:?}"),
                &format!("error: {e}"),
            ),
        }
        result
    }

    pub fn revoke_consent(&self, provider: &str) {
        self.gateway
            .router()
            .revoke_consent(&ProviderId::new(provider));
        self.record_audit("user", "consent", provider, "revoked");
    }

    pub fn consent_for(&self, provider: &str) -> Option<crate::model::ConsentRecord> {
        self.gateway
            .router()
            .consent_for(&ProviderId::new(provider))
    }
}

