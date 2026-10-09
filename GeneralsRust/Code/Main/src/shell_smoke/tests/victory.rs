//! Victory / match-over presentation residual tests.

pub use super::*;

#[test]
fn presentation_victory_summary_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
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
    fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source.find(signature).expect("source function signature");
        let open = source[start..].find('{').expect("source function body") + start;
        let mut depth = 0;
        for (offset, ch) in source[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[open + 1..open + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("unclosed source function: {signature}");
    }
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("build_with_victory")
            && eng.contains("Prefer presentation victory residual when frame installed")
            && eng.contains("Boot residual only — no presentation frame yet")
            && eng.contains("pres.match_over"),
        "InGame victory must prefer presentation match_over residual"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(pf.contains("fn build_with_victory"));
    let build = function_body(pf, "fn build_with_victory_with_tint_update(");
    assert!(
        build.contains("logic.current_victory_observation()")
            && build.contains("frame.match_over = victory.match_over")
            && build.contains("PresentationEvent::Victory")
            && build.contains("logic.build_victory_summary(winner)"),
        "snapshot must freeze the logic-owned victory observation and summary"
    );
    assert!(
        !build.contains("evaluate_victory_condition(") && !build.contains("kill_player("),
        "presentation must not evaluate victory or run defeat callbacks"
    );
}

#[cfg(test)]
mod presentation_victory_shell_tests {
    #[test]
    fn victory_eval_prefers_presentation_shell_bypass() {
        let eng = crate::cnc_game_engine::ENGINE_SRC;
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
