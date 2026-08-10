//! Victory / match-over presentation residual tests.

pub use super::*;

    #[test]
    fn presentation_victory_summary_residual() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        assert!(
            eng.contains("Prefer presentation-frozen summary when available")
                && eng.contains("Boot residual only — no presentation summary yet")
                && eng.contains("f.victory_summary.clone()"),
            "show_victory_screen must prefer presentation VictorySummary residual"
        );
        let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
        assert!(
            pf.contains("pub victory_summary:")
                && pf.contains("build_victory_summary(winner)")
                && pf.contains("fn victory_summary_residual"),
            "snapshot must freeze VictorySummary at evaluate"
        );
    }

    #[test]
    fn presentation_victory_prefers_snapshot_match_over() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        assert!(
            eng.contains("build_with_victory")
                && eng.contains("Prefer presentation victory residual when frame installed")
                && eng.contains("Boot residual only — no presentation frame yet")
                && eng.contains("pres.match_over"),
            "InGame victory must prefer presentation match_over residual"
        );
        let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
        assert!(
            pf.contains("fn build_with_victory")
                && pf.contains("evaluate_victory_condition")
                && pf.contains("PresentationEvent::Victory"),
            "snapshot must freeze victory residual once"
        );
    }

#[cfg(test)]
mod presentation_victory_shell_tests {
    #[test]
    fn victory_eval_prefers_presentation_shell_bypass() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        let idx = eng
            .find("Prefer presentation shell bypass when a frame is installed")
            .expect("victory shell prefer");
        let window = &eng[idx..idx + 500];
        assert!(
            window.contains("fow_shell_bypass") && window.contains("isInShellGame"),
            "victory eval must prefer presentation fow_shell_bypass with live residual"
        );
    }
}
