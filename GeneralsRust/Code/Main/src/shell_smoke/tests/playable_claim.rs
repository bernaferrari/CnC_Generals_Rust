//! playable_claim honesty: shell_host_playable_ok never implies retail.

pub use super::*;

    #[test]
    fn shell_host_playable_ok_never_implies_retail_playable_claim() {
        let r = run_shell_smoke(4);
        // Documented honesty contract: limited host flag is independent of retail claim.
        if r.shell_host_playable_ok {
            assert!(
                !r.playable_claim,
                "shell_host_playable_ok must never flip playable_claim"
            );
        }
        assert!(!r.playable_claim);
    }
