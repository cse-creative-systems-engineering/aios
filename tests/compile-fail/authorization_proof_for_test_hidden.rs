//! ADR-0010 §1.1 — the `for_test` escape hatch is `#[cfg(test)]`-gated on
//! the crate under test, so integration tests (compiled without that cfg
//! for the dependency crate) cannot see it.

use aios::broker::AuthorizationProof;
use aios::capability::RiskLevel;

fn main() {
    let _p = AuthorizationProof::for_test(RiskLevel::Staged);
    let _ = _p;
}
