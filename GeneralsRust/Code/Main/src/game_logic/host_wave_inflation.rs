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
        let table_only =
            "const NAMES: &[&str] = &[\"draw_construct_percent\", \"Wave 1115\", \"playable_claim = false\"];";
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
}
