//! ADR-0010 §1.1 — `AuthorizationProof` is the type-level witness that the
//! `PolicyBroker` allowed a request. Callers outside `broker.rs` must NOT
//! be able to fabricate one.
//!
//! Attack 1: private constructor.
//! (Attacks 2 and 3 live in separate files so each error is captured.)

use aios::broker::AuthorizationProof;
use aios::capability::RiskLevel;

fn main() {
    let _p = AuthorizationProof::new(uuid::Uuid::nil(), RiskLevel::Staged);
    let _ = _p;
}
