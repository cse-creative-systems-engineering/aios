//! Type-level TCB invariants (ADR-0010 §1.1).
//!
//! `AuthorizationProof` is the compile-time witness that the `PolicyBroker`
//! authorized a specific tool request. This test suite uses `trybuild` to
//! verify that no code outside `broker.rs` can synthesize one, by
//! attempting three plausible attacks in isolation.

#[test]
fn authorization_proof_is_not_synthesizable_outside_broker() {
    let t = trybuild::TestCases::new();
    // Attack 1: private constructor.
    t.compile_fail("tests/compile-fail/authorization_proof_not_synthesizable.rs");
    // Attack 2: struct-literal (private fields).
    t.compile_fail("tests/compile-fail/authorization_proof_struct_literal.rs");
    // Attack 3: `for_test` is `#[cfg(test)]`-gated on the crate under test
    // and invisible to integration tests.
    t.compile_fail("tests/compile-fail/authorization_proof_for_test_hidden.rs");
}
