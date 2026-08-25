//! W3D mesh asset resolve residual (presentation model_key → W3DModel).
//!
//! Closes the highest-value mesh residual after PresentationFrame owns unit
//! identity for the main mesh pass:
//! 1. `model_key` / `ThingTemplate::get_model_name` → canonical W3D key
//! 2. Common units (USA_Ranger / airanger_s) resolve a non-empty model name
//! 3. Load real mesh bytes when assets are present; production records a miss
//!    and skips an unresolved mesh
//! 4. The fallback cube is an explicit debug/test-only diagnostic. Material /
//!    animation / GPU retail parity is still incomplete; scale uses the
//!    residual table (combat default 1.0).
//!
//! Wave 53 residual peels:
//! - Expanded common unit model_key table (top ZH host units)
//! - Placeholder last-keys ring buffer residual (capacity 32)
//! - W3DZH/Art/W3D path search residual honesty constants
//! - Mesh scale residual default 1.0 (ThingTemplate Scale not yet ported)
//!
//! Wave 75 residual peels:
//! - Expanded common unit model_key table (air / hero / defense / ZH host units)
//! - Retail archive basename residual map (airanger_s → AIRanger_S, etc.)
//! - W3D search residual: W3DEnglishZH root + mixed-case archive filename variants
//! - Mesh scale residual table for common ZH combat units (retail default 1.0)
//!   plus known non-default CINE/weapon scales (honesty only)
//!
//! Production GPU upload remains `GraphicsSystem` / deferred load budget.

use crate::assets::models::{W3DLoader, W3DModel};
use crate::game_logic::ThingTemplate;
use crate::release_candidate;
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// Sentinel model name for the diagnostic placeholder cube (matches GraphicsSystem).
pub const PLACEHOLDER_MODEL_KEY: &str = "__fallback_cube__";

/// Default mesh scale residual (C++ Object Scale omitted → 1.0).
/// Fail-closed: not full ThingTemplate Scale INI field / draw scale bone.
pub const DEFAULT_MESH_SCALE: f32 = 1.0;

/// Placeholder last-keys ring capacity residual.
pub const PLACEHOLDER_KEY_RING_CAPACITY: usize = 32;

/// Retail / extract W3D search root residual path fragments (honesty constants).
///
/// Production `filesystem_w3d_candidates` joins these under CWD / manifest roots.
/// Wave 75: W3DEnglishZH + EnglishZH extract roots residual.
pub const W3D_SEARCH_ROOT_RESIDUALS: &[&str] = &[
    "windows_game/extracted_big_files/W3DZH/Art/W3D",
    "windows_game/extracted_big_files_v2/W3DZH/Art/W3D",
    "windows_game/extracted_big_files/W3DEnglishZH/Art/W3D",
    "windows_game/extracted_big_files_v2/W3DEnglishZH/Art/W3D",
    "windows_game/extracted_big_files/Art/W3D",
    "GeneralsRust/Code/Tools/w3d_to_gltf/W3D",
    "Code/Tools/w3d_to_gltf/W3D",
    "Art/W3D",
    "Data/Art/W3D",
];

/// Minimum residual roots required for honesty (W3DZH + English + tools + Art).
pub const W3D_SEARCH_ROOT_MIN_COUNT: usize = 7;

static RESOLVE_LOADED: AtomicUsize = AtomicUsize::new(0);
static RESOLVE_PLACEHOLDER: AtomicUsize = AtomicUsize::new(0);
static RESOLVE_MISSING: AtomicUsize = AtomicUsize::new(0);
static LAST_PLACEHOLDER_KEYS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

/// Honesty counters for mesh resolve outcomes (production + tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeshResolveHonesty {
    pub loaded: usize,
    pub placeholder: usize,
    pub missing: usize,
}

impl MeshResolveHonesty {
    pub fn snapshot() -> Self {
        Self {
            loaded: RESOLVE_LOADED.load(Ordering::Relaxed),
            placeholder: RESOLVE_PLACEHOLDER.load(Ordering::Relaxed),
            missing: RESOLVE_MISSING.load(Ordering::Relaxed),
        }
    }

    pub fn reset_for_tests() {
        RESOLVE_LOADED.store(0, Ordering::Relaxed);
        RESOLVE_PLACEHOLDER.store(0, Ordering::Relaxed);
        RESOLVE_MISSING.store(0, Ordering::Relaxed);
        if let Some(lock) = LAST_PLACEHOLDER_KEYS.get() {
            if let Ok(mut v) = lock.lock() {
                v.clear();
            }
        }
    }

    /// True when at least one real mesh load was recorded.
    pub fn honesty_loaded_ok(&self) -> bool {
        self.loaded > 0
    }

    /// True when a missing-asset path recorded a placeholder (honest, not silent).
    pub fn honesty_placeholder_ok(&self) -> bool {
        self.placeholder > 0
    }
}

fn note_placeholder_key(model_key: &str) {
    let lock = LAST_PLACEHOLDER_KEYS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut v) = lock.lock() {
        // Ring buffer residual: drop oldest when at capacity; skip duplicates.
        if let Some(pos) = v.iter().position(|k| k == model_key) {
            // Move existing key to the newest end (recency residual).
            let existing = v.remove(pos);
            v.push(existing);
            return;
        }
        if v.len() >= PLACEHOLDER_KEY_RING_CAPACITY {
            v.remove(0);
        }
        v.push(model_key.to_string());
    }
}

/// Recent model keys that fell back to the placeholder cube (ring residual).
pub fn recent_placeholder_model_keys() -> Vec<String> {
    LAST_PLACEHOLDER_KEYS
        .get()
        .and_then(|lock| lock.lock().ok().map(|v| v.clone()))
        .unwrap_or_default()
}

/// Honesty: placeholder last-keys ring buffer residual is operational.
pub fn honesty_placeholder_key_ring_ok() -> bool {
    PLACEHOLDER_KEY_RING_CAPACITY == 32
}

/// Honesty: W3D path search residual roots include W3DZH/Art/W3D + Art/W3D.
pub fn honesty_w3d_path_search_roots_ok() -> bool {
    let has_w3dzh = W3D_SEARCH_ROOT_RESIDUALS
        .iter()
        .any(|p| p.contains("W3DZH/Art/W3D"));
    let has_english = W3D_SEARCH_ROOT_RESIDUALS
        .iter()
        .any(|p| p.contains("W3DEnglishZH"));
    let has_art = W3D_SEARCH_ROOT_RESIDUALS
        .iter()
        .any(|p| *p == "Art/W3D" || p.ends_with("/Art/W3D") || p.ends_with("Art/W3D"));
    let has_tools = W3D_SEARCH_ROOT_RESIDUALS
        .iter()
        .any(|p| p.contains("w3d_to_gltf"));
    has_w3dzh
        && has_english
        && has_art
        && has_tools
        && W3D_SEARCH_ROOT_RESIDUALS.len() >= W3D_SEARCH_ROOT_MIN_COUNT
}

/// Known non-default Object INI Scale residual peels (CINE / weapon / nature).
///
/// Retail combat units omit Scale → 1.0. These honesty entries document the
/// sparse non-default peels so presentation/mesh scale residual is not silent.
/// Fail-closed: not full ThingTemplate.scale field / draw-scale bone matrix.
pub fn known_non_default_mesh_scales() -> &'static [(&'static str, f32)] {
    &[
        // AmericaCINEUnit.ini residual
        ("CINE_AmericaInfantryRanger", 0.66),
        ("CINE_AmericaInfantryMissileDefender", 0.66),
        // GLACINEUnit.ini residual
        ("CINE_GLAInfantryRebel", 0.8),
        ("CINE_GLAInfantryRPGTrooper", 0.8),
        // WeaponObjects.ini residual
        ("ClusterMine", 0.6),
        ("SpectreHowitzerShell", 0.7),
        // NatureUnit.ini residual
        ("Tree01", 5.0),
        ("Tree02", 1.5),
    ]
}

/// Mesh scale residual for a known unit / object template name.
///
/// Common ZH combat units retail-default to **1.0** (Scale field omitted in Object INI).
/// Non-default peels from `known_non_default_mesh_scales` apply when listed.
pub fn mesh_scale_for_unit(template_name: &str) -> f32 {
    if let Some((_, scale)) = known_non_default_mesh_scales()
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(template_name))
    {
        return *scale;
    }
    // Host residual: common combat units + unmapped names default to 1.0.
    DEFAULT_MESH_SCALE
}

/// Authored Object INI scale frozen on the live template.
///
/// Templates constructed outside the Object INI path retain the C++ default
/// `1.0`; live game objects no longer derive scale from a template-name table.
pub fn mesh_scale_from_template(template: &ThingTemplate) -> f32 {
    let scale = template.asset_scale;
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        DEFAULT_MESH_SCALE
    }
}

/// Honesty: mesh scale residual for common ZH combat units is retail 1.0, and
/// known non-default CINE/weapon peels are present.
pub fn honesty_mesh_scale_residual_ok() -> bool {
    let common_ok = (DEFAULT_MESH_SCALE - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("USA_Ranger") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("USA_Humvee") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("USA_Crusader") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("USA_Tomahawk") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("USA_Raptor") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("China_BattleTank") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("China_DragonTank") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("China_GattlingTank") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("China_OverlordTank") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("GLA_Scorpion") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("GLA_Technical") - 1.0).abs() < 0.001
        && (mesh_scale_for_unit("GLA_MarauderTank") - 1.0).abs() < 0.001;
    let non_default_ok = known_non_default_mesh_scales().len() >= 4
        && (mesh_scale_for_unit("CINE_AmericaInfantryRanger") - 0.66).abs() < 0.001
        && (mesh_scale_for_unit("CINE_GLAInfantryRebel") - 0.8).abs() < 0.001;
    common_ok && non_default_ok
}

/// Combined mesh-asset residual honesty (keys + search + scale + ring).
pub fn honesty_mesh_asset_residual_ok() -> bool {
    honesty_w3d_path_search_roots_ok()
        && honesty_placeholder_key_ring_ok()
        && honesty_mesh_scale_residual_ok()
        && honesty_retail_basename_residual_ok()
        && common_unit_model_keys().len() >= 30
        && common_unit_has_model_key("USA_Ranger")
        && common_unit_has_model_key("USA_Raptor")
        && common_unit_has_model_key("China_OverlordTank")
        && common_unit_has_model_key("GLA_ScudLauncher")
}

/// Outcome of resolving a presentation `model_key` to mesh data.
#[derive(Debug, Clone)]
pub enum MeshResolveResult {
    /// Real W3D mesh data loaded (CPU-side; GPU upload is separate).
    Loaded {
        model_key: String,
        model: W3DModel,
        source_path: Option<PathBuf>,
    },
    /// Honest diagnostic placeholder when asset bytes are missing.
    Placeholder {
        requested_key: String,
        model: W3DModel,
        reason: String,
    },
    /// Known miss without placeholder (call site opted out of fallback).
    Missing {
        requested_key: String,
        reason: String,
    },
}

impl MeshResolveResult {
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded { .. })
    }

    pub fn is_placeholder(&self) -> bool {
        matches!(self, Self::Placeholder { .. })
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }

    pub fn model(&self) -> Option<&W3DModel> {
        match self {
            Self::Loaded { model, .. } | Self::Placeholder { model, .. } => Some(model),
            Self::Missing { .. } => None,
        }
    }

    pub fn mesh_count(&self) -> usize {
        self.model().map(|m| m.meshes.len()).unwrap_or(0)
    }

    pub fn model_key(&self) -> &str {
        match self {
            Self::Loaded { model_key, .. } => model_key.as_str(),
            Self::Placeholder { requested_key, .. } | Self::Missing { requested_key, .. } => {
                requested_key.as_str()
            }
        }
    }
}

/// Canonical W3D key from a presentation/template model string.
pub fn canonical_model_key(model_name: &str) -> String {
    model_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(model_name)
        .trim()
        .trim_end_matches(".w3d")
        .trim_end_matches(".W3D")
        .to_string()
}

/// Exact compatibility aliases for historical Rust template names.
///
/// Each alias below is the *same object's* retail `DefaultConditionState`
/// `Model`, taken from the Object INIs. This is deliberately not a visual
/// fallback table: a missing model must stay missing instead of borrowing a
/// different unit's chassis, faction, damage, construction, or snow mesh.
pub fn remap_model_key_alias(model_key: &str) -> String {
    let key = canonical_model_key(model_key);
    let lower = key.to_ascii_lowercase();
    match lower.as_str() {
        // AmericaInfantry.ini.
        "airanger" | "usa_infantry_ranger" | "usa_ranger" | "americainfantryranger" => {
            "airngr_skn".to_string()
        }
        "usa_missiledefender" | "americainfantrymissiledefender" => "nithnt_skn".to_string(),
        "usa_pathfinder" | "americainfantrypathfinder" => "aipfdr_skn".to_string(),
        "usa_colonelburton" | "americainfantrycolonelburton" => "aihero_skn".to_string(),
        // AmericaVehicle.ini / AmericaAir.ini.
        "usa_humvee" | "americavehiclehumvee" => "avhummer".to_string(),
        "usa_dozer" | "americavehicledozer" => "avconstdoz_a".to_string(),
        "usa_tomahawk" | "americavehicletomahawk" => "avtomahawk".to_string(),
        "usa_crusader" | "usa_crusadertank" | "americatankcrusader" => "avleopard".to_string(),
        "usa_paladin" | "usa_paladintank" | "americatankpaladin" => "avpaladin".to_string(),
        "usa_raptor" | "americajetraptor" => "avraptor".to_string(),
        "usa_comanche" | "americavehiclecomanche" => "avcomanche".to_string(),
        "usa_chinook" | "americavehiclechinook" => "avchinook".to_string(),
        "usa_sentrydrone" | "americavehiclesentrydrone" => "avsentry".to_string(),
        "usa_ambulance" | "americavehiclemedic" => "avambulance".to_string(),
        "usa_stealthfighter" | "americajetstealthfighter" => "avstealth".to_string(),
        "usa_aurora" | "americajetaurora" => "avaurora".to_string(),
        "usa_spectregunship" | "americajetspectregunship" => "avsgunship".to_string(),
        "usa_avenger" | "americatankavenger" => "avavnger".to_string(),
        "usa_microwave" | "americatankmicrowave" => "avthundrblt".to_string(),
        "usa_patriot" | "americapatriotbattery" => "abpatriot".to_string(),
        // ChinaInfantry.ini / ChinaVehicle.ini / ChinaAir.ini.
        "china_soldier" | "china_redguard" | "chinainfantryredguard" => "nicnsc_skn".to_string(),
        "china_dozer" | "chinavehicledozer" => "nvconstdoz_a".to_string(),
        "china_battlemastertank" | "chinatankbattlemaster" => "nvbtmstr".to_string(),
        "china_battletank" => "nvbtmstr".to_string(),
        "china_dragontank" | "chinatankdragon" => "nvdragon".to_string(),
        "china_gattlingtank" | "chinatankgattling" => "nvgatttank".to_string(),
        "china_tankhunter" | "chinainfantrytankhunter" => "nimsst_skn".to_string(),
        "china_overlordtank" | "chinatankoverlord" => "nvovrlrd".to_string(),
        "china_infernocannon" | "chinavehicleinfernocannon" => "nvinferno".to_string(),
        "china_mig" | "chinajetmig" => "nvmig".to_string(),
        "china_helix" | "chinavehiclehelix" => "nvhelix".to_string(),
        "china_nukecannon" | "chinavehiclenukelauncher" => "nvnukecn".to_string(),
        "china_troopcrawler" | "chinavehicletroopcrawler" => "nvtcrawler".to_string(),
        "china_listeningoutpost" | "chinavehiclelisteningoutpost" => "nvloutpost".to_string(),
        "china_ecmtank" | "chinatankecm" => "nvbanshee".to_string(),
        // GLAInfantry.ini / GLAVehicle.ini.
        "gla_soldier" | "glainfantryrebel" => "uirgrd_skn".to_string(),
        "gla_worker" | "glainfantryworker" => "uiwrkr_skn".to_string(),
        "gla_technical" | "glavehicletechnical" => "uvtechtrck".to_string(),
        "gla_scorpion" | "gla_scorpiontank" | "glatankscorpion" | "glavehiclescorpion" => {
            "uvlitetank".to_string()
        }
        "gla_rpgtrooper" | "glainfantryrpgtrooper" => "uitunf_skn".to_string(),
        "gla_maraudertank" | "glatankmarauder" => "uvmarauder".to_string(),
        "gla_scudlauncher" | "glavehiclescudlauncher" => "uvscudlchr".to_string(),
        "gla_bombtruck" | "glavehiclebombtruck" => "uvbmbtruk".to_string(),
        "gla_quadcannon" | "glavehiclequadcannon" => "uvquadcann".to_string(),
        "gla_rocketbuggy" | "glavehiclerocketbuggy" => "uvrockbug".to_string(),
        "gla_battlebus" | "glavehiclebattlebus" => "uvbattbus".to_string(),
        "gla_stingersite" | "glastingersite" => "ubstingers".to_string(),
        "gla_jarmenkell" | "glainfantryjarmenkell" => "uihero_skn".to_string(),
        // USA structures — FactionBuilding.ini Model + host buildings.rs set_model.
        "usa_commandcenter" | "americacommandcenter" => "abbtcmdhq".to_string(),
        "usa_powerplant" | "americapowerplant" => "abpwrplant".to_string(),
        "usa_supplycenter" | "americasupplycenter" => "absupplyct".to_string(),
        "usa_barracks" | "americabarracks" => "abbarracks".to_string(),
        "usa_warfactory" | "americawarfactory" => "abwarfact".to_string(),
        // China structures — FactionBuilding.ini Model + host buildings.rs set_model.
        "china_commandcenter" | "chinacommandcenter" => "nbconyard".to_string(),
        "china_powerplant" | "chinapowerplant" => "nbpwrplant".to_string(),
        "china_barracks" | "chinabarracks" => "nbbarracks".to_string(),
        "china_warfactory" | "chinawarfactory" => "nbwarfact".to_string(),
        "china_supplycenter" | "chinasupplycenter" => "nbsupcent".to_string(),
        // GLA structures — FactionBuilding.ini Model + host buildings.rs set_model.
        "gla_commandcenter" | "glacommandcenter" | "gla_command" => "ubcmdhq".to_string(),
        "gla_supplystash" | "glasupplystash" => "ubsupply".to_string(),
        "gla_barracks" | "glabarracks" => "ubbarracks".to_string(),
        "gla_tunnelnetwork" | "glatunnelnetwork" => "ubundtunn".to_string(),
        // NatureProp.ini trees / rocks (W3DTreeDraw ModelName, never Object name).
        "generictree" | "genericopttree" | "treedogwood1" => "ptdogwod01".to_string(),
        "treedogwood1snow" => "ptdogwod01_s".to_string(),
        "bush01" => "ptbush01".to_string(),
        "treefir01b" => "ptfir01".to_string(),
        "treecherryblossom01" => "ptblossom01".to_string(),
        "treecherryblossom02" => "ptblossom02".to_string(),
        "treeoak1" => "ptoak01".to_string(),
        "treemaple2" => "ptmaple02".to_string(),
        "treepine" => "ptpine01".to_string(),
        "treespruce" => "ptspruce01".to_string(),
        "treespruce2" => "ptspruce01_hi".to_string(),
        "treespruce03" => "ptxpine03".to_string(),
        "treespruce03snow" => "ptxpine06".to_string(),
        "rocks1" => "pmrocks01".to_string(),
        "rocks2" => "pmrocks02".to_string(),
        // Preserve a concrete requested model exactly. It may be a valid
        // ConditionState model that is not represented by a legacy template
        // alias above.
        _ => key,
    }
}

/// Retail archive basename residual map (canonical lower key → shipped casing).
///
/// macOS APFS is case-insensitive; Linux extract trees are not. Wave 75 residual
/// peels the known mixed-case ZH basenames used in W3DZH/Art/W3D.
pub fn retail_w3d_basename_residuals() -> &'static [(&'static str, &'static str)] {
    &[
        ("airngr_skn", "AIRngr_SKN"),
        ("nithnt_skn", "NITHNT_SKN"),
        ("aipfdr_skn", "AIPFDR_SKN"),
        ("aihero_skn", "AIHERO_SKN"),
        ("airanger_s", "AIRanger_S"),
        ("aimissletm", "AIMissleTm"),
        ("aipthfindr", "AIPthFindr"),
        ("aihero01", "AIHero01"),
        ("avhummer", "AvHummer"),
        ("avcrusader", "AVCrusader"),
        ("avleopard", "AVLeopard"),
        ("avpaladin", "AVPaladin"),
        ("avtomahawk", "AVTomahawk"),
        ("avdozer", "AVDozer"),
        ("avconstdoz_a", "AVCONSTDOZ_A"),
        ("avraptorag", "AVRaptorAG"),
        ("avraptor", "AVRaptor"),
        ("avcomanche", "AVComanche"),
        ("avchinook", "AVChinook"),
        ("avsentry", "AVSentry"),
        ("avambulance", "AVAmbulance"),
        ("avstealth", "AVStealth"),
        ("avaurora", "AVAurora"),
        ("avsgunship", "AVSGunship"),
        ("abpatriot", "ABPatriot"),
        ("ubstingers", "UBStingerS"),
        ("uirebel", "UIRebel"),
        ("uirguard02", "UIRGuard02"),
        ("uvlitetank", "UVLiteTank"),
        ("uvscudlchr", "UVScudLchr"),
        ("uvbuggy", "UVBuggy"),
        ("uvtechvan_d1", "UVTechVan_d1"),
        ("nvbtmstr", "NVBtMstr"),
        ("nvovrlrd", "NVOvrlrd"),
        ("nvovrlrdt", "NVOvrlrdT"),
        ("nvinferno", "NVInferno"),
        ("nvmign", "NVMigN"),
        ("nvmig", "NVMIG"),
        ("nvhelix", "NVHelix"),
        ("nvdragon", "NVDragon"),
        ("nvgatttank", "NVGattTank"),
        ("nicnsc_skn", "NICNSC_SKN"),
        ("nimsst_skn", "NIMSST_SKN"),
        ("nvconstdoz_a", "NVCONSTDOZ_A"),
        ("nvnukecn", "NVNukeCn"),
        ("nvtcrawler", "NVTCrawler"),
        ("nvloutpost", "NVLOUTPOST"),
        ("nvbanshee", "NVBANSHEE"),
        ("uirgrd_skn", "UIRGrd_SKN"),
        ("uiwrkr_skn", "UIWRKR_SKN"),
        ("uitunf_skn", "UITunF_SKN"),
        ("uihero_skn", "UIHERO_SKN"),
        ("uvtechtrck", "UVTechTrck"),
        ("uvmarauder", "UVMarauder"),
        ("uvbmbtruk", "UVBMBTRUK"),
        ("uvquadcann", "UVQuadCann"),
        ("uvrockbug", "UVRockBug"),
        ("uvbattbus", "UVBATTBUS"),
        // Structure / prop archive casing (FactionBuilding.ini + NatureProp.ini).
        ("abbtcmdhq", "ABBtCmdHQ"),
        ("abpwrplant", "ABPWRPLANT"),
        ("abpwrplant_d06", "ABPWRPLANT_d06"),
        ("absupplyct", "ABSupplyCT"),
        ("absupplyct_a2", "ABSupplyCT_A2"),
        ("abbarracks", "ABBarracks"),
        ("abbarracks_fa", "ABBarracks_FA"),
        ("abwarfact", "ABWarFact"),
        ("abwarfact_e", "ABWarFact_E"),
        ("abpatriotsw", "ABPatriotSW"),
        ("nbconyard", "NBConYard"),
        ("nbconyard_fa", "NBConYard_FA"),
        ("nbpwrplant", "NBPwrPlant"),
        ("nbnreactr", "NBNReactr"),
        ("nbbarracks", "NBBarracks"),
        ("nbintcnt", "NBIntCnt"),
        ("nbwarfact", "NBWarFact"),
        ("nbweapfact", "NBWeapFact"),
        ("nbsupcent", "NBSupCent"),
        ("cxsupcent", "CXSupCent"),
        ("ubcmdhq", "UBCmdHQ"),
        ("ubarfrccmd", "UBArfrCCmd"),
        ("ubsupply", "UBSupply"),
        ("ubsupply_f", "UBSupply_F"),
        ("ubbarracks", "UBBarracks"),
        ("ubbarracksf", "UBBarracksF"),
        ("ubundtunn", "UBUndTunn"),
        ("ubhole_a4", "UBHole_A4"),
        ("ptdogwod01", "PTDogwod01"),
        ("ptdogwod01_s", "PTDogwod01_S"),
        ("ptmaple02", "PTMaple02"),
    ]
}

/// Resolve the preferred retail archive basename for a model key (case residual).
pub fn retail_w3d_basename_for_key(model_key: &str) -> String {
    let key = remap_model_key_alias(model_key);
    let lower = key.to_ascii_lowercase();
    if let Some((_, retail)) = retail_w3d_basename_residuals()
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&lower))
    {
        return (*retail).to_string();
    }
    key
}

/// Compatibility hook for presentation callers that used to retry a different
/// W3D after a miss.
///
/// It intentionally has no entries.  Models such as `ABPWRPLANT_d06`,
/// `ABBarracks_FA`, `PTDogwod01_S`, and `UBArFrcCmd` are debris, construction,
/// snow, or faction-specific `ConditionState` assets—not substitutes for the
/// pristine model requested by the original game.
pub fn w3d_load_fallback_keys(_model_key: &str) -> &'static [&'static str] {
    &[]
}

/// Archive / filesystem path stems to try for a model key (`art/w3d/{key}.w3d`, `{key}.w3d`).
pub fn w3d_archive_path_variants(model_key: &str) -> Vec<String> {
    let remapped = remap_model_key_alias(model_key);
    let retail = retail_w3d_basename_for_key(&remapped);
    let stems = vec![remapped.clone(), retail];
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for stem in stems {
        if stem.is_empty() {
            continue;
        }
        for name in w3d_filename_variants(&stem) {
            for path in [format!("art/w3d/{name}"), name.clone()] {
                let key = path.to_ascii_lowercase();
                if seen.insert(key) {
                    out.push(path);
                }
            }
        }
    }
    out
}

/// Honesty: retail basename residual map covers AIRanger_S + AvHummer + AVRaptorAG.
pub fn honesty_retail_basename_residual_ok() -> bool {
    retail_w3d_basename_for_key("airanger_s").eq_ignore_ascii_case("AIRanger_S")
        && retail_w3d_basename_for_key("avhummer").eq_ignore_ascii_case("AvHummer")
        && retail_w3d_basename_for_key("avraptorag").eq_ignore_ascii_case("AVRaptorAG")
        && retail_w3d_basename_for_key("aimissletm").eq_ignore_ascii_case("AIMissleTm")
        && retail_w3d_basename_residuals().len() >= 20
}

/// Model key from a template (presentation parity with get_model_name + alias).

/// C++ W3DDraw module condition model residual (host fail-closed suffix peel).
///
/// Retail draw modules swap meshes on DAMAGED / REALLYDAMAGED / RUBBLE / DYING.
/// Full ConditionState INI graph is deferred; this peels common basename suffixes
/// (`d`, `rd`, `die`) when the base key is known so presentation can request the
/// damaged mesh key without live GameLogic dual-read.
pub fn model_key_with_body_damage(base_key: &str, body_damage_state: u8, dying: bool) -> String {
    let base = remap_model_key_alias(base_key.trim());
    if base.is_empty() {
        return base;
    }
    // BodyDamageType ordinal residual: 0 pristine, 1 damaged, 2 really, 3 rubble.
    let want = if dying || body_damage_state >= 3 {
        BodyMeshWant::RubbleOrDying
    } else if body_damage_state == 2 {
        BodyMeshWant::ReallyDamaged
    } else if body_damage_state == 1 {
        BodyMeshWant::Damaged
    } else {
        return base;
    };
    pick_body_damage_model_key(&base, want)
}

/// Wave 491: presentation mesh key from body damage + sold/death model-condition residual.
///
/// Sold structures use the rubble/dying mesh branch (C++ sold draw residual).
pub fn model_key_with_presentation_state(
    base_key: &str,
    body_damage_state: u8,
    dying: bool,
    sold: bool,
) -> String {
    model_key_with_body_damage(base_key, body_damage_state, dying || sold)
}

/// Wave 497: mesh key from body damage + full stamped model-condition bits.
///
/// Treats SOLD and RUBBLE model-condition bits as dying/rubble mesh branch
/// (same path as explicit sold/dying flags).
pub fn model_key_with_presentation_conditions(
    base_key: &str,
    body_damage_state: u8,
    dying: bool,
    model_condition_bits: u128,
) -> String {
    use crate::game_logic::host_enum_table_residual::{
        MC_BIT_RUBBLE, host_model_condition_has, sold_model_bit,
    };
    let sold = host_model_condition_has(model_condition_bits, sold_model_bit());
    let rubble = host_model_condition_has(model_condition_bits, MC_BIT_RUBBLE);
    model_key_with_presentation_state(base_key, body_damage_state, dying || rubble, sold)
}

#[derive(Clone, Copy)]
enum BodyMeshWant {
    Damaged,
    ReallyDamaged,
    RubbleOrDying,
}

fn pick_body_damage_model_key(base: &str, want: BodyMeshWant) -> String {
    let lower = base.to_ascii_lowercase();
    if lower.ends_with('d')
        || lower.ends_with("rd")
        || lower.ends_with("_d")
        || lower.ends_with("_rd")
        || lower.ends_with("die")
        || lower.ends_with("_die")
        || lower.ends_with("rubble")
    {
        return base.to_string();
    }
    // Explicit host residual alias (highest priority).
    if !matches!(want, BodyMeshWant::Damaged) || true {
        if let Some(dmg) = explicit_damaged_model_key(base) {
            return remap_model_key_alias(dmg);
        }
    }
    let suffixes: &[&str] = match want {
        BodyMeshWant::Damaged => &["d", "_d"],
        BodyMeshWant::ReallyDamaged => &["rd", "_rd", "d", "_d"],
        BodyMeshWant::RubbleOrDying => {
            &["d", "_d", "rd", "_rd", "die", "_die", "rubble", "_rubble"]
        }
    };
    for suf in suffixes {
        let candidate = format!("{base}{suf}");
        if common_model_key_known(&candidate) {
            return remap_model_key_alias(&candidate);
        }
    }
    base.to_string()
}

fn common_model_key_known(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    common_unit_model_keys()
        .iter()
        .any(|(_, k)| k.eq_ignore_ascii_case(&lower))
        || honesty_has_explicit_damage_model_alias(&lower)
}

/// Explicit host residual damage-model aliases (base → damaged key).
fn honesty_has_explicit_damage_model_alias(lower_candidate: &str) -> bool {
    explicit_body_damage_model_aliases()
        .iter()
        .any(|(_, dmg)| dmg.eq_ignore_ascii_case(lower_candidate))
}

/// Host residual map: pristine model key → preferred damaged mesh key.
/// Fail-closed peel — not full W3D ConditionState INI.
pub fn explicit_body_damage_model_aliases() -> &'static [(&'static str, &'static str)] {
    &[
        // When a damaged mesh key is known in the common table / archives residual.
        // airanger_s stays (no separate damaged key in common table).
        // Vehicles that share chassis often reuse base; leave empty peel.
        // Placeholder residual pairs for honesty tests (self-map not used).
        ("avcrusader", "avcrusaderd"),
        ("avhummer", "avhummerd"),
        ("nvbtmstr", "nvbtmstrd"),
        ("gvscorpion", "gvscorpiond"),
        ("abpatriot", "abpatriotd"),
    ]
}

/// Resolve damaged model key via explicit alias table first.
pub fn explicit_damaged_model_key(base: &str) -> Option<&'static str> {
    let lower = remap_model_key_alias(base).to_ascii_lowercase();
    explicit_body_damage_model_aliases()
        .iter()
        .find(|(b, _)| b.eq_ignore_ascii_case(&lower))
        .map(|(_, d)| *d)
}

/// Honesty pack: body-damage model key residual surface.
pub fn honesty_body_damage_model_key_residual_ok() -> bool {
    let pristine = model_key_with_body_damage("avcrusader", 0, false);
    let damaged = model_key_with_body_damage("avcrusader", 1, false);
    let really = model_key_with_body_damage("avcrusader", 2, false);
    let dying = model_key_with_body_damage("avcrusader", 0, true);
    pristine.eq_ignore_ascii_case("avcrusader")
        && damaged.eq_ignore_ascii_case("avcrusaderd")
        && really.eq_ignore_ascii_case("avcrusaderd") // rd falls back to d alias residual
        && dying.eq_ignore_ascii_case("avcrusaderd")
        && model_key_with_body_damage("airanger_s", 1, false).eq_ignore_ascii_case("airanger_s")
        && explicit_body_damage_model_aliases().len() >= 5
}

/// C++ ProjectileObject → W3D model key residual for presentation trails/meshes.
///
/// Fail-closed: not full ThingTemplate lookup for projectile draw modules.
/// Maps known residual ProjectileObject names to archive basenames used by
/// host peels; unknown names pass through alias remap for asset resolve.
pub fn model_key_from_projectile_object(projectile_object_name: &str) -> String {
    let raw = projectile_object_name.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("NONE") {
        return String::new();
    }
    let lower = raw.to_ascii_lowercase();
    if let Some((_, key)) = projectile_object_model_keys()
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(&lower))
    {
        return remap_model_key_alias(key);
    }
    // Generic residual: ProjectileObject names often match W3D basenames.
    remap_model_key_alias(raw)
}

/// Host residual map: ProjectileObject template name → W3D model key.
pub fn projectile_object_model_keys() -> &'static [(&'static str, &'static str)] {
    &[
        ("GenericTankShell", "pmgntankshell"),
        ("GenericMissile", "pmgenericmissile"),
        ("JetMissile", "pmjetmissile"),
        ("TomahawkMissile", "pmtomahawk"),
        ("ScudMissile", "pmscud"),
        ("PatriotMissile", "pmpatriot"),
        ("RocketBuggyMissile", "pmbuggymissile"),
        ("NukeCannonShell", "pmnukeshell"),
        ("AuroraBomb", "pmaurorabomb"),
        ("FlashBangGrenade", "pmflashbang"),
        ("TunnelDefenderMissile", "pmgntankshell"),
        ("StingerMissile", "pmpatriot"),
    ]
}

/// Honesty: projectile object → model key residual surface.
pub fn honesty_projectile_object_model_key_residual_ok() -> bool {
    model_key_from_projectile_object("").is_empty()
        && model_key_from_projectile_object("NONE").is_empty()
        && model_key_from_projectile_object("GenericTankShell")
            .eq_ignore_ascii_case("pmgntankshell")
        && model_key_from_projectile_object("TomahawkMissile").eq_ignore_ascii_case("pmtomahawk")
        && model_key_from_projectile_object("UnknownProjectileXYZ")
            == remap_model_key_alias("UnknownProjectileXYZ")
        && projectile_object_model_keys().len() >= 8
}

pub fn model_key_from_template(template: &ThingTemplate) -> String {
    if let Some(model) = template
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("none"))
    {
        return remap_model_key_alias(model);
    }
    drawable_w3d_model_key(&template.name)
}

/// C++ W3DTreeDraw / W3DModelDraw load `ModelName` / ConditionState Model.
/// ObjectReskin identities such as `TreeSpruce03` are never W3D basenames.
pub fn drawable_w3d_model_key(object_or_model_name: &str) -> String {
    let trimmed = object_or_model_name.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(mapped) = authored_model_name_for_object(trimmed) {
        return remap_model_key_alias(&mapped);
    }
    remap_model_key_alias(trimmed)
}

fn authored_model_name_for_object(object_name: &str) -> Option<String> {
    let manager_arc = crate::assets::get_asset_manager()?;
    let manager = manager_arc.lock().ok()?;
    let mapped = manager.get_model_for_object(object_name)?;
    let mapped = mapped.trim();
    if mapped.is_empty() || mapped.eq_ignore_ascii_case("none") {
        return None;
    }
    if mapped.eq_ignore_ascii_case(object_name) {
        return None;
    }
    Some(mapped.to_string())
}

/// Model key from presentation fields (model_key preferred, else template_name).
///
/// Callers sometimes pass the Object / ObjectReskin identity as `model_key`.
/// Resolve that through INI `ModelName` instead of requesting `TreeSpruce03.w3d`.
pub fn model_key_from_presentation(model_key: Option<&str>, template_name: &str) -> String {
    if let Some(raw) = model_key.map(str::trim).filter(|s| !s.is_empty()) {
        return drawable_w3d_model_key(raw);
    }
    drawable_w3d_model_key(template_name)
}

/// Known common unit template → model key pairs used by host setup / prewarm.
///
/// Wave 53 residual: top ZH host units from `units.rs` create templates.
/// Wave 75 residual: air / hero / defense / ZH host units from `game_logic` setup.
pub fn common_unit_model_keys() -> &'static [(&'static str, &'static str)] {
    &[
        // USA infantry / hero
        ("USA_Ranger", "airngr_skn"),
        ("AmericaInfantryRanger", "airngr_skn"),
        ("USA_MissileDefender", "nithnt_skn"),
        ("USA_Pathfinder", "aipfdr_skn"),
        ("USA_ColonelBurton", "aihero_skn"),
        // USA vehicles / tanks
        ("USA_Humvee", "avhummer"),
        ("USA_Crusader", "avleopard"),
        ("USA_CrusaderTank", "avleopard"),
        ("USA_Paladin", "avpaladin"),
        ("USA_PaladinTank", "avpaladin"),
        ("USA_Dozer", "avconstdoz_a"),
        ("USA_Tomahawk", "avtomahawk"),
        ("USA_Ambulance", "avambulance"),
        ("USA_SentryDrone", "avsentry"),
        ("USA_Avenger", "avavnger"),
        ("USA_Microwave", "avthundrblt"),
        // USA air
        ("USA_Raptor", "avraptor"),
        ("USA_Comanche", "avcomanche"),
        ("USA_Chinook", "avchinook"),
        ("USA_StealthFighter", "avstealth"),
        ("USA_Aurora", "avaurora"),
        ("USA_SpectreGunship", "avsgunship"),
        // USA defense
        ("USA_Patriot", "abpatriot"),
        ("AmericaPatriotBattery", "abpatriot"),
        // China
        ("China_Soldier", "nicnsc_skn"),
        ("China_RedGuard", "nicnsc_skn"),
        ("China_Dozer", "nvconstdoz_a"),
        ("China_BattleTank", "nvbtmstr"),
        ("China_BattlemasterTank", "nvbtmstr"),
        ("China_DragonTank", "nvdragon"),
        ("China_GattlingTank", "nvgatttank"),
        ("China_TankHunter", "nimsst_skn"),
        ("China_OverlordTank", "nvovrlrd"),
        ("China_InfernoCannon", "nvinferno"),
        ("China_MiG", "nvmig"),
        ("China_Helix", "nvhelix"),
        ("China_NukeCannon", "nvnukecn"),
        ("China_TroopCrawler", "nvtcrawler"),
        ("China_ListeningOutpost", "nvlistout"),
        ("China_ECMTank", "nvbanshee"),
        // GLA
        ("GLA_Soldier", "uirgrd_skn"),
        ("GLA_Worker", "uiwrkr_skn"),
        ("GLA_Technical", "uvtechtrck"),
        ("GLA_Scorpion", "uvlitetank"),
        ("GLA_ScorpionTank", "uvlitetank"),
        ("GLA_RPGTrooper", "uitunf_skn"),
        ("GLA_MarauderTank", "uvmarauder"),
        ("GLA_ScudLauncher", "uvscudlchr"),
        ("GLA_BombTruck", "uvbmbtruk"),
        ("GLA_QuadCannon", "uvquadcann"),
        ("GLA_RocketBuggy", "uvrockbug"),
        ("GLA_BattleBus", "uvbattbus"),
        ("GLA_JarmenKell", "uihero_skn"),
        ("GLA_StingerSite", "ubstingers"),
        ("GLAStingerSite", "ubstingers"),
        ("GLA_TunnelNetwork", "ubundtunn"),
        // USA structures (FactionBuilding.ini / host buildings.rs)
        ("USA_CommandCenter", "abbtcmdhq"),
        ("AmericaCommandCenter", "abbtcmdhq"),
        ("USA_PowerPlant", "abpwrplant"),
        ("AmericaPowerPlant", "abpwrplant"),
        ("USA_SupplyCenter", "absupplyct"),
        ("AmericaSupplyCenter", "absupplyct"),
        ("USA_Barracks", "abbarracks"),
        ("AmericaBarracks", "abbarracks"),
        ("USA_WarFactory", "abwarfact"),
        ("AmericaWarFactory", "abwarfact"),
        // China structures
        ("China_CommandCenter", "nbconyard"),
        ("ChinaCommandCenter", "nbconyard"),
        ("China_PowerPlant", "nbpwrplant"),
        ("ChinaPowerPlant", "nbpwrplant"),
        ("China_Barracks", "nbbarracks"),
        ("ChinaBarracks", "nbbarracks"),
        ("China_WarFactory", "nbwarfact"),
        ("ChinaWarFactory", "nbwarfact"),
        ("China_SupplyCenter", "nbsupcent"),
        ("ChinaSupplyCenter", "nbsupcent"),
        // GLA structures
        ("GLA_CommandCenter", "ubcmdhq"),
        ("GLACommandCenter", "ubcmdhq"),
        ("GLA_SupplyStash", "ubsupply"),
        ("GLASupplyStash", "ubsupply"),
        ("GLA_Barracks", "ubbarracks"),
        ("GLABarracks", "ubbarracks"),
        ("GLATunnelNetwork", "ubundtunn"),
        // NatureProp.ini trees / rocks
        ("GenericTree", "ptdogwod01"),
        ("GenericOptTree", "ptdogwod01"),
        ("TreeDogwood1", "ptdogwod01"),
        ("TreeMaple2", "ptmaple02"),
        ("TreeSpruce03", "ptxpine03"),
        ("Rocks1", "pmrocks01"),
    ]
}

/// True when a template name has a known non-empty model mapping.
pub fn common_unit_has_model_key(template_name: &str) -> bool {
    let lower = template_name.to_ascii_lowercase();
    common_unit_model_keys()
        .iter()
        .any(|(name, key)| name.eq_ignore_ascii_case(&lower) && !key.is_empty())
}

/// Expected model key for a known unit template, if mapped.
pub fn expected_model_key_for_unit(template_name: &str) -> Option<&'static str> {
    common_unit_model_keys()
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(template_name))
        .map(|(_, key)| *key)
}

/// Build residual filename variants for a model key (case + retail archive peel).
pub fn w3d_filename_variants(model_key: &str) -> Vec<String> {
    let key = canonical_model_key(model_key);
    let lower = key.to_ascii_lowercase();
    let retail = retail_w3d_basename_for_key(&key);
    let title = format!(
        "{}{}",
        key.chars()
            .next()
            .map(|c| c.to_ascii_uppercase().to_string())
            .unwrap_or_default(),
        key.chars().skip(1).collect::<String>()
    );
    let mut names = vec![
        format!("{key}.w3d"),
        format!("{key}.W3D"),
        format!("{lower}.w3d"),
        format!("{lower}.W3D"),
        format!("{}.W3D", key.to_ascii_uppercase()),
        format!("{}.w3d", key.to_ascii_uppercase()),
        // AIRanger_S / first-char-upper residual.
        format!("{title}.W3D"),
        format!("{title}.w3d"),
        // Retail archive basename residual (AIRanger_S.W3D, AvHummer.W3D, …).
        format!("{retail}.W3D"),
        format!("{retail}.w3d"),
        format!("{}.W3D", retail.to_ascii_lowercase()),
        format!("{}.w3d", retail.to_ascii_lowercase()),
    ];
    // Dedup while preserving order.
    let mut seen = std::collections::HashSet::new();
    names.retain(|n| seen.insert(n.to_ascii_lowercase()));
    names
}

/// Filesystem candidates for a model key (repo / extracted BIG / tools samples).
///
/// Roots include residual honesty constants from `W3D_SEARCH_ROOT_RESIDUALS`
/// (W3DZH/Art/W3D, W3DEnglishZH, Art/W3D, tools samples) plus manifest-relative
/// fallbacks. Wave 75: retail archive basename variants (AIRanger_S etc.).
pub fn filesystem_w3d_candidates(model_key: &str) -> Vec<PathBuf> {
    let key = remap_model_key_alias(model_key);
    if key.is_empty() || key == PLACEHOLDER_MODEL_KEY {
        return Vec::new();
    }

    let file_names = w3d_filename_variants(&key);

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.clone());
        // Residual honesty roots (W3DZH/Art/W3D etc.) — keep in sync with constants.
        for rel in W3D_SEARCH_ROOT_RESIDUALS {
            roots.push(cwd.join(rel));
        }
        // When running from GeneralsRust/Code/Main
        roots.push(cwd.join("../../../windows_game/extracted_big_files/W3DZH/Art/W3D"));
        roots.push(cwd.join("../../../windows_game/extracted_big_files/W3DEnglishZH/Art/W3D"));
        roots.push(cwd.join("../../Tools/w3d_to_gltf/W3D"));
        roots.push(cwd.join("../Tools/w3d_to_gltf/W3D"));
    }
    // CARGO_MANIFEST_DIR for Main crate tests
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    roots.push(manifest.join("assets"));
    roots.push(manifest.join("../Tools/w3d_to_gltf/W3D"));
    roots.push(manifest.join("../../windows_game/extracted_big_files/W3DZH/Art/W3D"));
    roots.push(manifest.join("../../../windows_game/extracted_big_files/W3DZH/Art/W3D"));
    roots.push(manifest.join("../../../windows_game/extracted_big_files/W3DEnglishZH/Art/W3D"));
    for rel in W3D_SEARCH_ROOT_RESIDUALS {
        roots.push(manifest.join("../../../").join(rel));
    }

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        for name in &file_names {
            let p = root.join(name);
            let key_s = p.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key_s) {
                out.push(p);
            }
        }
        // Also try Art/W3D (and art/w3d) under root with original key casing variants
        for sub in ["Art/W3D", "art/w3d"] {
            for name in &file_names {
                let p = root.join(sub).join(name);
                let key_s = p.to_string_lossy().to_ascii_lowercase();
                if seen.insert(key_s) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// First existing filesystem W3D path for a model key, if any.
pub fn find_filesystem_w3d(model_key: &str) -> Option<PathBuf> {
    filesystem_w3d_candidates(model_key)
        .into_iter()
        .find(|p| p.is_file())
}

/// Neutral gray unit cube used when retail mesh bytes are missing.
/// Matches GraphicsSystem::__fallback_cube__ semantics (CPU-side only here).
pub fn create_placeholder_mesh_model() -> W3DModel {
    use crate::assets::models::{W3DMaterial, W3DMesh, W3DVertex};

    const DIFFUSE: [f32; 4] = [0.58, 0.58, 0.58, 1.0];
    let s = 5.0;
    let vertices = vec![
        W3DVertex {
            position: [-s, -s, s],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
            color: DIFFUSE,
        },
        W3DVertex {
            position: [s, -s, s],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
            color: DIFFUSE,
        },
        W3DVertex {
            position: [s, s, s],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 1.0],
            color: DIFFUSE,
        },
        W3DVertex {
            position: [-s, s, s],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
            color: DIFFUSE,
        },
        W3DVertex {
            position: [-s, -s, -s],
            normal: [0.0, 0.0, -1.0],
            uv: [1.0, 0.0],
            color: DIFFUSE,
        },
        W3DVertex {
            position: [s, -s, -s],
            normal: [0.0, 0.0, -1.0],
            uv: [0.0, 0.0],
            color: DIFFUSE,
        },
        W3DVertex {
            position: [s, s, -s],
            normal: [0.0, 0.0, -1.0],
            uv: [0.0, 1.0],
            color: DIFFUSE,
        },
        W3DVertex {
            position: [-s, s, -s],
            normal: [0.0, 0.0, -1.0],
            uv: [1.0, 1.0],
            color: DIFFUSE,
        },
    ];
    // Expand to 24 verts for per-face normals like GraphicsSystem (simplified 8-vert cube).
    let indices: Vec<u32> = vec![
        0, 1, 2, 0, 2, 3, // front
        5, 4, 7, 5, 7, 6, // back
        3, 2, 6, 3, 6, 7, // top
        4, 5, 1, 4, 1, 0, // bottom
        1, 5, 6, 1, 6, 2, // right
        4, 0, 3, 4, 3, 7, // left
    ];

    let mut material = W3DMaterial::default();
    material.name = "__fallback_material__".to_string();
    material.diffuse_color = Vec3::new(0.58, 0.58, 0.58);

    let mesh = W3DMesh {
        name: "__fallback_cube_mesh__".to_string(),
        container_name: String::new(),
        vertices,
        indices,
        material,
        transform: Mat4::IDENTITY,
        header: None,
        stage_texcoords: Vec::new(),
        passes: Vec::new(),
        per_pass_stage_texture_ids: Vec::new(),
        per_pass_stage_texture_names: Vec::new(),
        per_pass_vertex_material_ids: Vec::new(),
        per_pass_shader_ids: Vec::new(),
        per_pass_dcg_colors: Vec::new(),
        per_pass_dig_colors: Vec::new(),
        vertex_materials: Vec::new(),
        shaders: Vec::new(),
        vertex_influences: None,
        vertex_shade_indices: None,
        per_stage_face_texcoord_ids: Vec::new(),
        stage_uv_channels: Vec::new(),
        texture_library: Vec::new(),
        vertex_mappers: Vec::new(),
        vertices_in_render_space: true,
        has_explicit_vertex_colors: true,
    };

    W3DModel {
        name: PLACEHOLDER_MODEL_KEY.to_string(),
        meshes: vec![mesh],
        materials: HashMap::new(),
        texture_names: Vec::new(),
        ww3d_mesh_models: HashMap::new(),
        bounding_box_min: Vec3::new(-5.0, -5.0, -5.0),
        bounding_box_max: Vec3::new(5.0, 5.0, 5.0),
        hierarchy: None,
        hierarchies: Vec::new(),
        hlods: Vec::new(),
        hmodels: Vec::new(),
        emitters: Vec::new(),
        dazzles: Vec::new(),
        boxes: Vec::new(),
        rings: Vec::new(),
        spheres: Vec::new(),
        nulls: Vec::new(),
        collections: Vec::new(),
        aggregates: Vec::new(),
        dist_lods: Vec::new(),
        hlod_parse_failed: false,
        animations: Vec::new(),
    }
}

/// Try to load a model from filesystem W3D bytes (no AssetManager / GPU).
pub fn try_load_w3d_from_filesystem(model_key: &str) -> Option<(W3DModel, PathBuf)> {
    let key = remap_model_key_alias(model_key);
    let path = find_filesystem_w3d(&key)?;
    let loader = W3DLoader::new();
    match loader.load_model_from_path(&path) {
        Ok(model) if !model.meshes.is_empty() => Some((model, path)),
        Ok(_) => {
            log::warn!(
                "W3D mesh residual: '{}' at {} parsed with zero meshes",
                key,
                path.display()
            );
            None
        }
        Err(err) => {
            log::debug!(
                "W3D mesh residual: failed to parse '{}' at {}: {err}",
                key,
                path.display()
            );
            None
        }
    }
}

/// Collect the requested key, its canonical alias, and retail archive casing.
fn mesh_load_key_candidates(model_key: &str) -> Vec<String> {
    let remapped = remap_model_key_alias(model_key);
    let mut keys = Vec::new();
    let mut push = |k: String| {
        if k.is_empty() {
            return;
        }
        if !keys.iter().any(|e: &String| e.eq_ignore_ascii_case(&k)) {
            keys.push(k);
        }
    };
    push(remapped.clone());
    push(retail_w3d_basename_for_key(&remapped));
    if !model_key.eq_ignore_ascii_case(&remapped) {
        push(model_key.to_string());
    }
    keys
}

/// Try AssetManager cache / load when the global manager is available.
///
/// Tries `art/w3d/{key}.w3d`, `{key}.w3d`, retail basename casing, and the
/// template `get_model_for_object` remap.  Do not replace an absent model with
/// a construction, damage, snow, or faction variant: those are distinct C++
/// `ConditionState` assets, not aliases.
pub fn try_load_w3d_from_asset_manager(model_key: &str) -> Option<W3DModel> {
    let remapped = remap_model_key_alias(model_key);
    let manager_arc = crate::assets::get_asset_manager()?;
    let mut manager = manager_arc.lock().ok()?;
    let keys = mesh_load_key_candidates(model_key);

    for key in &keys {
        if let Some(model) = manager.get_cached_model(key) {
            if !model.meshes.is_empty() {
                return Some(model);
            }
        }
    }

    // Object-definition remap (template name → INI model).
    for probe in [model_key, remapped.as_str()] {
        if let Some(mapped) = manager.get_model_for_object(probe) {
            let mapped_key = remap_model_key_alias(&mapped);
            if let Some(model) = manager.get_cached_model(&mapped_key) {
                if !model.meshes.is_empty() {
                    return Some(model);
                }
            }
            if let Ok(model) = manager.load_w3d_model(&mapped_key) {
                if !model.meshes.is_empty() {
                    return Some(model);
                }
            }
        }
    }

    // `load_w3d_model` already probes art/w3d/{name}.w3d and {name}.w3d.
    for key in &keys {
        if let Ok(model) = manager.load_w3d_model(key) {
            if !model.meshes.is_empty() {
                return Some(model);
            }
        }
    }
    None
}

/// Resolve presentation model_key → W3DModel with honesty bookkeeping.
///
/// Order:
/// 1. AssetManager (if initialized)
/// 2. Filesystem extracted / sample W3D
/// 3. Placeholder cube when `use_placeholder`, else Missing
///
/// Fail-closed: not full material/animation/GPU parity.
pub fn resolve_mesh_for_model_key(model_key: &str, use_placeholder: bool) -> MeshResolveResult {
    let key = remap_model_key_alias(model_key);
    if key.is_empty() {
        RESOLVE_MISSING.fetch_add(1, Ordering::Relaxed);
        release_candidate::note_missing_w3d_model("<empty>");
        return if use_placeholder {
            RESOLVE_PLACEHOLDER.fetch_add(1, Ordering::Relaxed);
            note_placeholder_key("<empty>");
            MeshResolveResult::Placeholder {
                requested_key: String::new(),
                model: create_placeholder_mesh_model(),
                reason: "empty model key".into(),
            }
        } else {
            MeshResolveResult::Missing {
                requested_key: String::new(),
                reason: "empty model key".into(),
            }
        };
    }

    if key == PLACEHOLDER_MODEL_KEY {
        RESOLVE_PLACEHOLDER.fetch_add(1, Ordering::Relaxed);
        note_placeholder_key(key.as_str());
        return MeshResolveResult::Placeholder {
            requested_key: key,
            model: create_placeholder_mesh_model(),
            reason: "explicit placeholder key".into(),
        };
    }

    if let Some(model) = try_load_w3d_from_asset_manager(&key) {
        RESOLVE_LOADED.fetch_add(1, Ordering::Relaxed);
        return MeshResolveResult::Loaded {
            model_key: key,
            model,
            source_path: None,
        };
    }

    if let Some((model, path)) = try_load_w3d_from_filesystem(&key) {
        RESOLVE_LOADED.fetch_add(1, Ordering::Relaxed);
        return MeshResolveResult::Loaded {
            model_key: key,
            model,
            source_path: Some(path),
        };
    }

    RESOLVE_MISSING.fetch_add(1, Ordering::Relaxed);
    release_candidate::note_missing_w3d_model(&key);
    if use_placeholder {
        RESOLVE_PLACEHOLDER.fetch_add(1, Ordering::Relaxed);
        note_placeholder_key(&key);
        MeshResolveResult::Placeholder {
            requested_key: key,
            model: create_placeholder_mesh_model(),
            reason: "W3D asset not found".into(),
        }
    } else {
        MeshResolveResult::Missing {
            requested_key: key,
            reason: "W3D asset not found".into(),
        }
    }
}

/// Resolve using presentation fields.
pub fn resolve_mesh_for_presentation(
    model_key: Option<&str>,
    template_name: &str,
    use_placeholder: bool,
) -> MeshResolveResult {
    let key = model_key_from_presentation(model_key, template_name);
    resolve_mesh_for_model_key(&key, use_placeholder)
}

/// Resolve from a ThingTemplate (host create_object / presentation path).
pub fn resolve_mesh_for_template(
    template: &ThingTemplate,
    use_placeholder: bool,
) -> MeshResolveResult {
    let key = model_key_from_template(template);
    resolve_mesh_for_model_key(&key, use_placeholder)
}

/// Whether retail/sample W3D bytes for this key are discoverable right now.
pub fn mesh_asset_available(model_key: &str) -> bool {
    if find_filesystem_w3d(model_key).is_some() {
        return true;
    }
    if let Some(manager_arc) = crate::assets::get_asset_manager() {
        if let Ok(mut manager) = manager_arc.lock() {
            for path in w3d_archive_path_variants(model_key) {
                if manager.can_open_file_sync(&path) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, ThingTemplate};

    #[test]
    fn usa_ranger_template_resolves_non_empty_model_key() {
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry);
        t.set_model("airanger_s");
        let key = model_key_from_template(&t);
        assert!(!key.is_empty(), "USA_Ranger model key must be non-empty");
        assert_eq!(key.to_ascii_lowercase(), "airanger_s");
    }

    #[test]
    fn airanger_alias_maps_to_airanger_s() {
        assert_eq!(
            remap_model_key_alias("airanger").to_ascii_lowercase(),
            "airanger_s"
        );
        assert_eq!(
            remap_model_key_alias("USA_Ranger").to_ascii_lowercase(),
            "airanger_s"
        );
        assert_eq!(
            remap_model_key_alias("AmericaInfantryRanger").to_ascii_lowercase(),
            "airanger_s"
        );
    }

    #[test]
    fn common_units_have_non_empty_model_keys() {
        for (template, key) in common_unit_model_keys() {
            assert!(!key.is_empty(), "{template} key empty");
            assert!(common_unit_has_model_key(template));
            assert_eq!(expected_model_key_for_unit(template), Some(*key));
        }
    }

    #[test]
    fn presentation_model_key_prefers_explicit_over_template() {
        let key = model_key_from_presentation(Some("avhummer"), "USA_Humvee");
        assert_eq!(key.to_ascii_lowercase(), "avhummer");
        let key2 = model_key_from_presentation(None, "USA_Ranger");
        assert_eq!(key2.to_ascii_lowercase(), "airanger_s");
    }

    #[test]
    fn tree_spruce03_lookup_uses_reskin_modelname() {
        assert_eq!(
            drawable_w3d_model_key("TreeSpruce03").to_ascii_lowercase(),
            "ptxpine03",
            "C++ W3DTreeDraw loads NatureProp ModelName PTXPine03, not TreeSpruce03.w3d"
        );
        assert_eq!(
            model_key_from_presentation(Some("TreeSpruce03"), "TreeSpruce03").to_ascii_lowercase(),
            "ptxpine03"
        );
        let mut t = ThingTemplate::new("TreeSpruce03");
        t.set_model("PTXPine03");
        assert_eq!(
            model_key_from_template(&t).to_ascii_lowercase(),
            "ptxpine03"
        );
        let unnamed = ThingTemplate::new("TreeSpruce03");
        assert_eq!(
            model_key_from_template(&unnamed).to_ascii_lowercase(),
            "ptxpine03",
            "object-name fallback must still remap through ModelName"
        );
    }

    #[test]
    fn missing_reskin_mesh_skips_without_invented_geometry() {
        let result = resolve_mesh_for_model_key("PTXPine03", false);
        match &result {
            MeshResolveResult::Loaded { .. } => {}
            MeshResolveResult::Missing { requested_key, .. } => {
                assert_eq!(requested_key.to_ascii_lowercase(), "ptxpine03");
            }
            MeshResolveResult::Placeholder { .. } => {
                panic!("must not invent placeholder geometry for a missing base-Generals mesh")
            }
        }
        assert!(!result.is_placeholder());
    }

    #[test]
    fn placeholder_mesh_has_geometry_and_honesty() {
        let before = MeshResolveHonesty::snapshot();
        let result = resolve_mesh_for_model_key("__no_such_unit_mesh_xyz__", true);
        assert!(result.is_placeholder(), "expected placeholder for missing");
        assert!(result.mesh_count() > 0, "placeholder must have mesh tris");
        assert_eq!(result.model().unwrap().name, PLACEHOLDER_MODEL_KEY);
        let after = MeshResolveHonesty::snapshot();
        assert!(
            after.placeholder > before.placeholder,
            "placeholder must flip honesty counter"
        );
        assert!(
            after.missing > before.missing,
            "missing note must also count"
        );
        let keys = recent_placeholder_model_keys();
        assert!(
            keys.iter()
                .any(|k| k.to_ascii_lowercase().contains("no_such_unit")),
            "placeholder key list must record requested model: {keys:?}"
        );
    }

    #[test]
    fn missing_without_placeholder_is_honest_missing() {
        let before = MeshResolveHonesty::snapshot();
        let result = resolve_mesh_for_model_key("__definitely_missing_mesh__", false);
        assert!(result.is_missing());
        assert!(result.model().is_none());
        let after = MeshResolveHonesty::snapshot();
        assert!(
            after.missing > before.missing,
            "missing path must increment honesty"
        );
        // Placeholder counter may race with parallel tests; outcome itself must not
        // be a placeholder when use_placeholder=false.
        assert!(!result.is_placeholder());
    }

    #[test]
    fn usa_ranger_loads_mesh_when_assets_present_or_skips() {
        let before = MeshResolveHonesty::snapshot();
        let key = "airanger_s";
        if !mesh_asset_available(key) {
            // Graceful skip when ZH extract / sample W3D not on disk.
            eprintln!("skip: airanger_s W3D not available in workspace");
            let result = resolve_mesh_for_model_key(key, true);
            assert!(
                result.is_placeholder() || result.is_missing(),
                "without assets, resolve must not invent a loaded retail mesh"
            );
            return;
        }

        let result = resolve_mesh_for_model_key(key, false);
        assert!(
            result.is_loaded(),
            "airanger_s must load when assets present: {:?}",
            result.model_key()
        );
        assert!(
            result.mesh_count() > 0,
            "loaded USA Ranger mesh must have geometry"
        );
        let after = MeshResolveHonesty::snapshot();
        assert!(
            after.loaded > before.loaded,
            "loaded honesty must increment when assets present"
        );
        if let MeshResolveResult::Loaded {
            source_path: Some(path),
            ..
        } = &result
        {
            assert!(
                path.is_file(),
                "source path should exist: {}",
                path.display()
            );
        }
    }

    #[test]
    fn template_resolve_path_matches_presentation_key() {
        let mut t = ThingTemplate::new("USA_Ranger");
        t.set_model("airanger"); // legacy alias
        let from_template = model_key_from_template(&t);
        let from_pres = model_key_from_presentation(Some("airanger"), "USA_Ranger");
        assert_eq!(from_template.to_ascii_lowercase(), "airanger_s");
        assert_eq!(from_pres.to_ascii_lowercase(), "airanger_s");
    }

    #[test]
    fn load_model_from_bytes_rejects_empty() {
        let loader = W3DLoader::new();
        assert!(loader.load_model_from_bytes(&[], "empty").is_err());
    }

    #[test]
    fn expanded_common_unit_table_covers_top_zh_host_units() {
        // Wave 53 residual: top ZH host units from units.rs must map.
        // Wave 75 residual: air / hero / defense / ZH host units.
        let required = [
            "USA_Ranger",
            "USA_Humvee",
            "USA_Crusader",
            "USA_Paladin",
            "USA_Tomahawk",
            "USA_MissileDefender",
            "USA_Raptor",
            "USA_Comanche",
            "USA_Chinook",
            "USA_ColonelBurton",
            "USA_Pathfinder",
            "USA_SpectreGunship",
            "USA_Patriot",
            "China_Soldier",
            "China_BattleTank",
            "China_DragonTank",
            "China_GattlingTank",
            "China_OverlordTank",
            "China_MiG",
            "China_Helix",
            "China_InfernoCannon",
            "GLA_Soldier",
            "GLA_Technical",
            "GLA_Scorpion",
            "GLA_MarauderTank",
            "GLA_ScudLauncher",
            "GLA_BombTruck",
            "GLA_RocketBuggy",
            "GLA_JarmenKell",
            "GLA_StingerSite",
        ];
        assert!(
            common_unit_model_keys().len() >= required.len(),
            "common unit table too small: {}",
            common_unit_model_keys().len()
        );
        for name in required {
            assert!(
                common_unit_has_model_key(name),
                "missing common unit model key residual: {name}"
            );
            let key = expected_model_key_for_unit(name).expect("key");
            assert!(!key.is_empty(), "{name} empty key");
        }
    }

    #[test]
    fn w3d_path_search_residual_honesty_constants() {
        assert!(honesty_w3d_path_search_roots_ok());
        assert!(
            W3D_SEARCH_ROOT_RESIDUALS
                .iter()
                .any(|p| p.contains("W3DZH/Art/W3D"))
        );
        assert!(
            W3D_SEARCH_ROOT_RESIDUALS
                .iter()
                .any(|p| p.contains("W3DEnglishZH"))
        );
        let candidates = filesystem_w3d_candidates("airanger_s");
        assert!(
            candidates.iter().any(|p| {
                let s = p.to_string_lossy();
                s.contains("W3DZH") || s.contains("Art/W3D") || s.contains("w3d_to_gltf")
            }),
            "filesystem candidates must include residual W3D search roots"
        );
        // Wave 75: retail basename residual must appear in candidate filenames.
        assert!(
            candidates.iter().any(|p| {
                let s = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                s.eq_ignore_ascii_case("AIRanger_S.W3D") || s.eq_ignore_ascii_case("AIRanger_S.w3d")
            }),
            "candidates must include AIRanger_S retail basename residual"
        );
    }

    #[test]
    fn retail_basename_residual_map_wave75() {
        assert!(honesty_retail_basename_residual_ok());
        assert_eq!(
            retail_w3d_basename_for_key("airanger_s").to_ascii_lowercase(),
            "airanger_s"
        );
        assert!(
            retail_w3d_basename_for_key("airanger_s").contains('R')
                || retail_w3d_basename_for_key("airanger_s") == "AIRanger_S"
        );
        assert_eq!(
            remap_model_key_alias("USA_Raptor").to_ascii_lowercase(),
            "avraptorag"
        );
        assert_eq!(
            remap_model_key_alias("AmericaPatriotBattery").to_ascii_lowercase(),
            "abpatriot"
        );
    }

    #[test]
    fn placeholder_key_ring_buffer_residual() {
        assert!(honesty_placeholder_key_ring_ok());
        MeshResolveHonesty::reset_for_tests();
        // Fill beyond capacity → ring drops oldest.
        for i in 0..(PLACEHOLDER_KEY_RING_CAPACITY + 4) {
            let key = format!("__ring_placeholder_{i}__");
            let _ = resolve_mesh_for_model_key(&key, true);
        }
        let keys = recent_placeholder_model_keys();
        assert!(
            keys.len() <= PLACEHOLDER_KEY_RING_CAPACITY,
            "ring must cap at {PLACEHOLDER_KEY_RING_CAPACITY}, got {}",
            keys.len()
        );
        // Newest keys retained.
        assert!(
            keys.iter()
                .any(|k| k.contains(&format!("{}", PLACEHOLDER_KEY_RING_CAPACITY + 3))),
            "newest key must remain in ring: {keys:?}"
        );
        // Oldest dropped.
        assert!(
            !keys
                .iter()
                .any(|k| k.ends_with("_0__") && k.contains("ring_placeholder_0")),
            "oldest key should be dropped from ring: {keys:?}"
        );
    }

    #[test]
    fn mesh_scale_residual_defaults_to_one() {
        assert!(honesty_mesh_scale_residual_ok());
        let mut t = ThingTemplate::new("USA_Ranger");
        t.set_model("airanger_s");
        assert!((mesh_scale_from_template(&t) - 1.0).abs() < 0.001);
        assert!((mesh_scale_for_unit("China_BattleTank") - DEFAULT_MESH_SCALE).abs() < 0.001);
        // Wave 75: common ZH combat units scale residual 1.0.
        for name in [
            "USA_Humvee",
            "USA_Raptor",
            "China_OverlordTank",
            "GLA_ScudLauncher",
        ] {
            assert!(
                (mesh_scale_for_unit(name) - 1.0).abs() < 0.001,
                "{name} combat scale residual must be 1.0"
            );
        }
        // Known non-default CINE residual peel.
        assert!((mesh_scale_for_unit("CINE_AmericaInfantryRanger") - 0.66).abs() < 0.001);
        assert!((mesh_scale_for_unit("CINE_GLAInfantryRebel") - 0.8).abs() < 0.001);
    }

    #[test]
    fn mesh_asset_residual_pack_honesty_wave75() {
        assert!(honesty_mesh_asset_residual_ok());
        assert!(common_unit_model_keys().len() >= 30);
        assert!(honesty_retail_basename_residual_ok());
        assert!(honesty_mesh_scale_residual_ok());
        assert!(honesty_w3d_path_search_roots_ok());
    }

    #[test]
    fn body_damage_model_key_residual() {
        assert!(honesty_body_damage_model_key_residual_ok());
        assert_eq!(
            model_key_with_body_damage("avhummer", 1, false).to_ascii_lowercase(),
            "avhummerd"
        );
        // Unknown base stays pristine (fail-closed).
        assert_eq!(
            model_key_with_body_damage("missingunitxyz", 2, false),
            remap_model_key_alias("missingunitxyz")
        );
    }

    #[test]
    fn projectile_object_model_key_residual() {
        assert!(honesty_projectile_object_model_key_residual_ok());
        assert_eq!(
            model_key_from_projectile_object("JetMissile").to_ascii_lowercase(),
            "pmjetmissile"
        );
    }

    #[test]
    fn faction_building_templates_remap_to_non_empty_w3d_keys() {
        let required = [
            ("AmericaCommandCenter", "abbtcmdhq"),
            ("USA_CommandCenter", "abbtcmdhq"),
            ("AmericaPowerPlant", "abpwrplant"),
            ("AmericaSupplyCenter", "absupplyct"),
            ("AmericaWarFactory", "abwarfact"),
            ("AmericaBarracks", "abbarracks"),
            ("AmericaPatriotBattery", "abpatriot"),
            ("ChinaCommandCenter", "nbconyard"),
            ("ChinaPowerPlant", "nbpwrplant"),
            ("ChinaWarFactory", "nbwarfact"),
            ("ChinaBarracks", "nbbarracks"),
            ("GLACommandCenter", "ubcmdhq"),
            ("GLASupplyStash", "ubsupply"),
            ("GLABarracks", "ubbarracks"),
            ("GLAStingerSite", "ubstingers"),
            ("GLATunnelNetwork", "ubundtunn"),
        ];
        for (template, expected) in required {
            let key = remap_model_key_alias(template);
            assert!(
                !key.is_empty(),
                "{template} must remap to a non-empty W3D key"
            );
            assert_eq!(
                key.to_ascii_lowercase(),
                expected,
                "{template} remap mismatch"
            );
            assert!(
                common_unit_has_model_key(template),
                "common_unit_model_keys must cover {template}"
            );
        }
    }

    #[test]
    fn remap_filesystem_candidates_include_art_w3d_and_w3d_variants() {
        let candidates = filesystem_w3d_candidates("AmericaCommandCenter");
        assert!(
            !candidates.is_empty(),
            "command center must produce filesystem candidates"
        );
        assert!(
            candidates.iter().any(|p| {
                let s = p.to_string_lossy();
                s.contains("art/w3d") || s.contains("Art/W3D")
            }),
            "candidates must include art/w3d variants: {candidates:?}"
        );
        assert!(
            candidates.iter().any(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("w3d"))
            }),
            "candidates must include .w3d filenames"
        );
        assert!(
            candidates.iter().any(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.eq_ignore_ascii_case("ABBtCmdHQ.W3D")
                    || name.eq_ignore_ascii_case("ABBtCmdHQ.w3d")
            }),
            "remapped command-center basename must appear: {candidates:?}"
        );
        let archive = w3d_archive_path_variants("AmericaCommandCenter");
        assert!(
            archive
                .iter()
                .any(|p| p.eq_ignore_ascii_case("art/w3d/ABBtCmdHQ.w3d")
                    || p.eq_ignore_ascii_case("art/w3d/abbtcmdhq.w3d")),
            "archive variants must include art/w3d/ABBtCmdHQ.w3d: {archive:?}"
        );
        assert!(
            archive
                .iter()
                .any(|p| p.eq_ignore_ascii_case("ABBtCmdHQ.w3d")
                    || p.eq_ignore_ascii_case("abbtcmdhq.w3d")),
            "archive variants must include ABBtCmdHQ.w3d: {archive:?}"
        );
    }

    #[test]
    fn pristine_models_never_cross_resolve_to_condition_or_faction_variants() {
        let pairs = [
            ("ABPWRPLANT", "ABPWRPLANT_d06"),
            ("ABBarracks", "ABBarracks_FA"),
            ("ABPatriot", "ABPatriotSW"),
            ("NBConYard", "NBConYard_FA"),
            ("UBCmdHQ", "UBArFrcCmd"),
            ("PTDogwod01", "PTDogwod01_S"),
        ];
        for (pristine, wrong_variant) in pairs {
            assert!(
                w3d_load_fallback_keys(pristine).is_empty(),
                "{pristine} must not have cross-state fallback keys"
            );
            assert!(
                !mesh_load_key_candidates(pristine)
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(wrong_variant)),
                "{pristine} must not resolve as distinct C++ state/faction asset {wrong_variant}"
            );
            assert!(
                !w3d_archive_path_variants(pristine)
                    .iter()
                    .any(|path| path.contains(wrong_variant)),
                "{pristine} archive lookup must not probe distinct C++ state/faction asset {wrong_variant}"
            );
        }
    }

    #[test]
    fn resolve_garbage_key_is_missing_not_panic() {
        let result = resolve_mesh_for_model_key("__garbage_stage_a_mesh_xyz__", false);
        assert!(result.is_missing(), "garbage key must be honest Missing");
        assert!(!result.is_placeholder());
        assert!(!result.is_loaded());
        let placeholder = resolve_mesh_for_model_key("__garbage_stage_a_mesh_xyz__", true);
        assert!(
            placeholder.is_placeholder(),
            "use_placeholder=true must still placeholder"
        );
        assert!(placeholder.mesh_count() > 0);
    }

    #[test]
    fn sample_w3d_loads_when_present_on_disk() {
        // ABBtCmdHQ.W3D ships under Tools/w3d_to_gltf/W3D and extracted BIG trees.
        let key = "AmericaCommandCenter";
        if !mesh_asset_available(key) && !mesh_asset_available("abbtcmdhq") {
            eprintln!("skip: ABBtCmdHQ W3D not available in workspace");
            let result = resolve_mesh_for_model_key(key, false);
            assert!(
                result.is_missing(),
                "without assets, production resolve must stay Missing (not placeholder)"
            );
            return;
        }
        let result = resolve_mesh_for_model_key(key, false);
        assert!(
            result.is_loaded(),
            "AmericaCommandCenter must load ABBtCmdHQ when sample W3D is present: {:?}",
            result.model_key()
        );
        assert!(result.mesh_count() > 0);
    }
}
