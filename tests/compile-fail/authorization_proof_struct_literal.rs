//! ADR-0010 §1.1 — attempt to synthesize an `AuthorizationProof` via
//! struct-literal syntax. All fields are private, so this must fail.

use aios::broker::AuthorizationProof;
use aios::capability::RiskLevel;

fn main() {
    let _p = AuthorizationProof {
        request_id: uuid::Uuid::nil(),
        risk: RiskLevel::Staged,
        _private: std::marker::PhantomData,
    };
    let _ = _p;
}
