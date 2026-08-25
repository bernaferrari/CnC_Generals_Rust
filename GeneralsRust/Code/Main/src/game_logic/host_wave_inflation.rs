//! Residual-wave honesty inflation policy.
//!
//! Many `honesty_*_method_names_residual_wave*` / `honesty_*_nav_commands_residual_wave*`
//! functions return true by scanning their **own** const tables:
//! `residual_name_index(names, "Wave 1089")`. That is not honesty — the table
//! always contains the names it declares.
//!
//! Residual packs that only source-scan comments plus `playable_claim = false`
//! in the residual file itself also inflate. Honesty must source-scan a **real
//! shipped function** (`include_str!` of GameClient / GameLogic / Main
//! production code) or return false.
//!
//! Residual packs must never publish retail `playable_claim`.
//!
//! This module (and other `*wave*` / `*residual*` audit packs) is compiled only
//! under `#[cfg(any(test, feature = "host-residuals"))]`. Production default
//! `cargo check -p generals_main --bin generals` must not `mod` those files.

/// Policy lock: self-table membership is inflation, not honesty.
///
/// Callers / tests assert this is `true` so a residual cannot claim honesty
/// solely via `residual_name_index` on its own `const` name/nav tables.
pub fn self_table_honesty_is_inflation() -> bool {
    true
}

/// Residual packs may keep source-scan honesty flags, but they must never
/// publish a retail `playable_claim`. Returns `true` iff the claim stayed false.
pub fn residual_pack_cannot_set_playable_claim(playable_claim: bool) -> bool {
    !playable_claim
}

/// Shared name-index helper. Looking up a name in the **same** residual's
/// const table is still inflation when that is the only honesty check.
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// True when `src` contains a shipped `fn <fn_name>` (definition, not a table string).
pub fn shipped_fn_exists(src: &str, fn_name: &str) -> bool {
    src.contains(&format!("fn {fn_name}"))
}

/// Window starting at a shipped function signature, if present.
pub fn shipped_fn_window<'a>(src: &'a str, fn_sig: &str, window: usize) -> Option<&'a str> {
    let i = src.find(fn_sig)?;
    Some(&src[i..src.len().min(i.saturating_add(window))])
}

/// Honesty helper: every `needle` must appear inside the shipped function window.
/// Missing function → false (fail-closed, not a green self-table lie).
pub fn shipped_fn_contains(src: &str, fn_sig: &str, needles: &[&str]) -> bool {
    match shipped_fn_window(src, fn_sig, 4000) {
        Some(body) => needles.iter().all(|n| body.contains(*n)),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_wave_inflation_self_table_honesty_is_inflation_policy() {
        assert!(
            self_table_honesty_is_inflation(),
            "scanning a residual's own const table is inflation, not honesty"
        );
    }

    #[test]
    fn host_wave_inflation_residual_pack_cannot_set_playable_claim() {
        assert!(
            residual_pack_cannot_set_playable_claim(false),
            "playable_claim=false satisfies residual-pack policy"
        );
        assert!(
            !residual_pack_cannot_set_playable_claim(true),
            "residual packs must not be allowed to publish playable_claim=true"
        );
    }

    #[test]
    fn host_wave_inflation_shipped_fn_contains_rejects_self_table_only() {
        let table_only = "const NAMES: &[&str] = &[\"draw_construct_percent\", \"Wave 1115\", \"playable_claim = false\"];";
        assert!(
            !shipped_fn_contains(
                table_only,
                "fn draw_construct_percent",
                &["presentation_sold"]
            ),
            "self-table membership must not count as shipped-fn honesty"
        );
        assert!(!shipped_fn_exists(table_only, "draw_construct_percent"));

        let real = "fn draw_construct_percent(&mut self) { if self.presentation_sold { return; } }";
        assert!(shipped_fn_exists(real, "draw_construct_percent"));
        assert!(shipped_fn_contains(
            real,
            "fn draw_construct_percent",
            &["presentation_sold"]
        ));
    }

    #[test]
    fn host_wave_inflation_production_default_does_not_mod_wave_files() {
        // Source-scan game_logic listings: audit wave/residual mods are cfg-gated
        // so the default `generals` binary does not compile them.
        let src = include_str!("mod.rs");
        let listing_src = concat!(
            include_str!("mod.rs"),
            include_str!("host_mods_combat.rs"),
            include_str!("host_mods_logs_a.rs"),
            include_str!("host_mods_logs_b.rs"),
            include_str!("host_mods_logs_c.rs"),
            include_str!("host_mods_special_powers.rs"),
            include_str!("host_mods_structures.rs"),
            include_str!("host_mods_residuals_on.rs"),
            include_str!("host_mods_units.rs"),
        );
        let cfg = r#"#[cfg(any(test, feature = "host-residuals"))]"#;
        assert!(
            src.contains(cfg),
            "game_logic/mod.rs must cfg-gate residual/wave audit modules"
        );

        for always in [
            "pub mod host_float_update;",
            "pub mod host_combat_attack_log;",
            "pub mod host_microwave;",
        ] {
            let idx = listing_src
                .find(always)
                .unwrap_or_else(|| panic!("missing {always}"));
            // Skip a `#[path = "..."]` line so we inspect the real cfg (if any).
            let before = listing_src[..idx].trim_end();
            let mut prev_lines = before.rsplit('\n');
            let mut prev = prev_lines.next().unwrap_or("").trim();
            if prev.starts_with("#[path") {
                prev = prev_lines.next().unwrap_or("").trim();
            }
            assert_ne!(
                prev, cfg,
                "{always} is real host gameplay and must stay in the default build"
            );
        }

        for audit in ["pub mod host_wave_inflation;"] {
            let idx = listing_src
                .find(audit)
                .unwrap_or_else(|| panic!("missing {audit}"));
            let before = listing_src[..idx].trim_end();
            let mut prev_lines = before.rsplit('\n');
            let mut prev = prev_lines.next().unwrap_or("").trim();
            if prev.starts_with("#[path") {
                prev = prev_lines.next().unwrap_or("").trim();
            }
            assert_eq!(
                prev, cfg,
                "{audit} must not be compiled into the default generals binary"
            );
        }

        // Wave/residual audit packs live under residuals/ and are re-exported
        // as crate::game_logic::host_* only when the cfg is on.
        let residuals_decl = "mod residuals;";
        let idx = src
            .find(residuals_decl)
            .unwrap_or_else(|| panic!("missing {residuals_decl}"));
        let prev = src[..idx].trim_end().rsplit('\n').next().unwrap_or("");
        let prev2 = src[..idx]
            .trim_end()
            .rsplit('\n')
            .nth(1)
            .unwrap_or("")
            .trim();
        assert_eq!(
            prev.trim(),
            r#"#[path = "residuals/mod.rs"]"#,
            "residuals must load from residuals/mod.rs"
        );
        assert_eq!(
            prev2, cfg,
            "residual/wave audit packs must be cfg-gated via residuals/"
        );
        assert!(
            src.contains("pub use residuals::*;"),
            "residuals must be re-exported as crate::game_logic::*"
        );
        assert!(
            !src.contains("pub mod host_live_exec_smoke_early_combat_residual_wave864;"),
            "audit packs must not be declared directly in game_logic/mod.rs"
        );
        let residuals_src = include_str!("residuals/mod.rs");
        assert!(
            residuals_src.contains("pub mod host_live_exec_smoke_early_combat_residual_wave864;"),
            "audit packs must be declared in residuals/mod.rs"
        );
    }
}
