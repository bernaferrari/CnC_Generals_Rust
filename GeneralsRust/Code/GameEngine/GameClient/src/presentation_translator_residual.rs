//! Wave 973: presentation residual for host message-stream translators.
//!
//! When OBJECT_REGISTRY is empty, translators answer relationship/kind/mine
//! queries from the stamped unit catalog residual instead of dual-world factory
//! objects. playable_claim stays false.

use std::sync::RwLock;

use once_cell::sync::Lazy;

/// Thin catalog entry for translator residual queries.
#[derive(Debug, Clone)]
pub struct TranslatorCatalogEntry {
    pub object_id: u32,
    /// Wave 976: template residual for host drawable template resolve.
    pub template_name: String,
    pub team_name: String,
    pub selectable: bool,
    pub kind_names: Vec<String>,
    pub special_power_ready: bool,
    /// Wave 974: world position residual for host context pick.
    pub position: [f32; 3],
    /// Wave 979: airborne residual for host plane-camera lock cycle.
    pub airborne_target: bool,
    /// Wave 981: FOW residual for host translators / command hints.
    pub shroud_status: u8,
    /// Wave 982: slaver residual for IgnoredInGui host mouseover.
    pub slaver_object_id: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct TranslatorPresentationResidual {
    pub local_team_name: String,
    pub catalog: Vec<TranslatorCatalogEntry>,
}

static RESIDUAL: Lazy<RwLock<TranslatorPresentationResidual>> =
    Lazy::new(|| RwLock::new(TranslatorPresentationResidual::default()));

/// Stamp host presentation residual used by message-stream translators.
pub fn set_translator_presentation_residual(
    local_team_name: impl Into<String>,
    catalog: Vec<TranslatorCatalogEntry>,
) {
    if let Ok(mut guard) = RESIDUAL.write() {
        guard.local_team_name = local_team_name.into();
        guard.catalog = catalog;
    }
}

pub fn translator_local_team_name() -> String {
    RESIDUAL
        .read()
        .ok()
        .map(|g| g.local_team_name.clone())
        .unwrap_or_default()
}

pub fn translator_catalog_entry(object_id: u32) -> Option<TranslatorCatalogEntry> {
    RESIDUAL
        .read()
        .ok()
        .and_then(|g| g.catalog.iter().find(|e| e.object_id == object_id).cloned())
}

pub fn translator_catalog_has_kind(object_id: u32, kind_name: &str) -> bool {
    translator_catalog_entry(object_id)
        .map(|e| {
            e.kind_names
                .iter()
                .any(|k| k == kind_name || k.eq_ignore_ascii_case(kind_name))
        })
        .unwrap_or(false)
}

pub fn translator_entry_is_local(entry: &TranslatorCatalogEntry) -> bool {
    let local = translator_local_team_name();
    !local.is_empty() && entry.team_name == local
}

pub fn translator_entry_has_kind(entry: &TranslatorCatalogEntry, kind_name: &str) -> bool {
    entry
        .kind_names
        .iter()
        .any(|k| k == kind_name || k.eq_ignore_ascii_case(kind_name))
}

/// Wave 974: iterate stamped catalog residual.
pub fn with_translator_catalog<F, R>(f: F) -> R
where
    F: FnOnce(&[TranslatorCatalogEntry]) -> R,
{
    let guard = RESIDUAL.read().ok();
    match guard {
        Some(g) => f(&g.catalog),
        None => f(&[]),
    }
}
