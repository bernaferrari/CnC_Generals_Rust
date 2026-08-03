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
    /// Wave 1024: yaw residual for dual-world drawable pose peel.
    pub orientation: f32,
    /// Wave 1026: disabled residual for dual-world command availability.
    pub disabled: bool,
    /// Wave 1028: under-construction residual.
    pub under_construction: bool,
    /// Wave 1028: construction percent residual [0,1].
    pub construction_percent: f32,
    /// Wave 1030: garrison capacity residual.
    pub max_garrison: u16,
    /// Wave 1030: occupant count residual.
    pub occupant_count: u16,
    /// Wave 1031: OCL timer residual seconds.
    pub ocl_timer_seconds: u32,
    /// Wave 1033: sold residual.
    pub sold: bool,
    /// Wave 1034: unselectable residual.
    pub unselectable: bool,
    /// Wave 1035: destroyed residual.
    pub destroyed: bool,
    /// Wave 1035: masked residual.
    pub masked: bool,
    /// Wave 979: airborne residual for host plane-camera lock cycle.
    pub airborne_target: bool,
    /// Wave 981: FOW residual for host translators / command hints.
    pub shroud_status: u8,
    /// Wave 982: slaver residual for IgnoredInGui host mouseover.
    pub slaver_object_id: Option<u32>,
    /// Wave 1011: health residual.
    pub health_current: f32,
    /// Wave 1011: max health residual.
    pub health_maximum: f32,
    /// Wave 1012: veterancy chevron residual.
    pub veterancy_overlay: Option<String>,
    /// Wave 1013: head production progress residual (0..1).
    pub production_progress: Option<f32>,
    /// Wave 1013: head production template residual.
    pub production_template: Option<String>,
    /// Wave 1013: production paused residual.
    pub production_paused: bool,
    /// Wave 1015: effective command-set name residual.
    pub command_set_name: String,
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

/// Wave 1005: dual-world residual — stamped presentation catalog size.
pub fn translator_catalog_len() -> usize {
    RESIDUAL.read().ok().map(|g| g.catalog.len()).unwrap_or(0)
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
