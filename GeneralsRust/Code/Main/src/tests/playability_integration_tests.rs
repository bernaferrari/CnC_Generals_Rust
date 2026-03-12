#![cfg(test)]

use crate::playability_integration::{current_unresolved_blocker_examples, PlayabilityAuditSummary};

#[test]
fn playability_integration_summary_smoke() {
    let summary = PlayabilityAuditSummary::default();
    assert_eq!(summary.total_parity_percent(), 0.0);
    assert!(summary.total_parity_percent() < 1.0);
    assert!(current_unresolved_blocker_examples(&summary, 5).is_empty());
}
