// Mechanical extract from cnc_game_engine.rs `interactive_playability_evidence_tests`.
// Child module via `#[path]`.

    use super::InteractivePlayabilityEvidence;

    #[test]
    fn scripted_or_hover_only_input_cannot_claim_retail_navigation() {
        let mut evidence = InteractivePlayabilityEvidence::default();
        evidence.note_menu_wnd_click(true, true, false);
        evidence.note_offline_match_started(true, true);
        evidence.note_gameplay_order(true, true);

        assert!(!evidence.wnd_menu_to_match_complete());
        assert!(!evidence.gameplay_complete());
    }

    #[test]
    fn physical_offline_menu_to_match_and_order_completes_evidence() {
        let mut evidence = InteractivePlayabilityEvidence::default();
        evidence.note_menu_wnd_click(true, true, true);
        evidence.note_offline_match_started(true, true);
        evidence.note_gameplay_order(true, true);

        assert!(evidence.wnd_menu_to_match_complete());
        assert!(evidence.gameplay_complete());
    }

    #[test]
    fn network_or_headless_paths_do_not_complete_retail_evidence() {
        let mut evidence = InteractivePlayabilityEvidence::default();
        evidence.note_menu_wnd_click(false, true, true);
        evidence.note_offline_match_started(true, false);
        evidence.note_gameplay_order(false, true);

        assert!(!evidence.wnd_menu_to_match_complete());
        assert!(!evidence.gameplay_complete());
    }
