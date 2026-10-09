//! C++ `UpgradeCenter` / `TheUpgradeCenter` (Upgrade.h, Upgrade.cpp:196-473).
//!
//! The single canonical upgrade-definition store. Every `Upgrade` INI block
//! (Default/Upgrade.ini, Upgrade.ini, map.ini) goes through
//! [`UpgradeCenter::parse_upgrade_definition`], and every mask bit is
//! assigned by [`UpgradeCenter::new_upgrade`] in allocation order — exactly
//! one allocator, as in C++.
//!
//! Original C++ Author: Colin Day, March 2002

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

use super::{UPGRADE_MAX_COUNT, UpgradeMask, UpgradeTemplate};
use crate::common::ascii_string::AsciiString;
use crate::common::ini::{INI, INIError};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};

/// C++ `findUpgrade("DefaultUpgrade")` name used by `newUpgrade`.
const DEFAULT_UPGRADE_NAME: &str = "DefaultUpgrade";

/// Central registry for upgrade templates (C++ `UpgradeCenter`).
#[derive(Clone, Default)]
pub struct UpgradeCenter {
    /// Exact authored name -> template. C++ looks up by
    /// `NAMEKEY(name)`; `nameToKey` is case-sensitive, so exact-name lookup
    /// is the same identity without depending on a thread's key namespace.
    upgrades: HashMap<String, Arc<UpgradeTemplate>>,
    /// C++ `m_upgradeList`: head-inserted, so the newest template is first.
    upgrade_list: Vec<Arc<UpgradeTemplate>>,
    /// C++ `m_nextTemplateMaskBit`.
    next_template_mask_bit: u32,
    /// C++ `buttonImagesCached` — one-shot during reset.
    button_images_cached: bool,
}

impl UpgradeCenter {
    /// C++ `UpgradeCenter::UpgradeCenter`.
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ `UpgradeCenter::init` (Upgrade.cpp:234-254): the three OBJECT
    /// veterancy upgrades take mask bits 0, 1, 2 before any INI is parsed.
    /// Rust worlds snapshot an already-initialized center, so a repeated
    /// `init` keeps the existing templates instead of duplicating them.
    pub fn init(&mut self) {
        for level in ["VETERAN", "ELITE", "HEROIC"] {
            let name = format!("Upgrade_Veterancy_{level}");
            if self.find_upgrade(&name).is_none() {
                self.create_veterancy_upgrade(level);
            }
        }
    }

    /// C++ `UpgradeCenter::reset` — cache button images once.
    pub fn reset(&mut self) {
        if self.button_images_cached {
            return;
        }
        if crate::common::ini::ini_mapped_image::get_mapped_image_collection().is_none() {
            return;
        }
        let list = std::mem::take(&mut self.upgrade_list);
        for template in list {
            let mut owned = (*template).clone();
            owned.cache_button_image();
            self.store_template(Arc::new(owned));
        }
        self.button_images_cached = true;
    }

    /// C++ `UpgradeCenter::newUpgrade` (Upgrade.cpp:330-358): copy
    /// `DefaultUpgrade` (current contents) when present, assign the name and
    /// the next unused mask bit, and head-insert into the list.
    ///
    /// Rust deviation: a non-empty name that already exists returns the
    /// existing template instead of allocating a duplicate. C++ callers
    /// (`parseUpgradeDefinition`) always look up first, so this only makes
    /// repeated programmatic registration idempotent.
    pub fn new_upgrade(&mut self, name: AsciiString) -> Arc<UpgradeTemplate> {
        if !name.is_empty() {
            if let Some(existing) = self.find_upgrade(name.as_str()) {
                return existing;
            }
        }

        let mut template = match self.find_upgrade(DEFAULT_UPGRADE_NAME) {
            Some(default) => (*default).clone(),
            None => UpgradeTemplate::new(name.clone()),
        };
        template.set_name(name);

        let bit = self.next_template_mask_bit as usize;
        self.next_template_mask_bit = self.next_template_mask_bit.saturating_add(1);
        let mut mask = UpgradeMask::none();
        if bit < UPGRADE_MAX_COUNT {
            mask.set_bit(bit);
        } else {
            log::error!(
                "Can't have over {UPGRADE_MAX_COUNT} types of Upgrades and have a Bitfield function ('{}')",
                template.get_name()
            );
        }
        template.friend_set_upgrade_mask(mask);

        let template = Arc::new(template);
        self.upgrades
            .insert(template.get_name().as_str().to_owned(), template.clone());
        self.upgrade_list.insert(0, template.clone());
        template
    }

    fn create_veterancy_upgrade(&mut self, level: &str) {
        // C++: up = newUpgrade(""); up->friend_makeVeterancyUpgrade(level);
        let template = self.new_upgrade(AsciiString::from(""));
        self.upgrades.remove(template.get_name().as_str());
        let mut owned = (*template).clone();
        owned.friend_make_veterancy_upgrade(level);
        let owned = Arc::new(owned);
        self.upgrades
            .insert(owned.get_name().as_str().to_owned(), owned.clone());
        if let Some(slot) = self.upgrade_list.first_mut() {
            *slot = owned;
        }
    }

    /// Replace an existing template in place (same list position and mask).
    fn store_template(&mut self, template: Arc<UpgradeTemplate>) {
        let name = template.get_name().as_str().to_owned();
        match self
            .upgrade_list
            .iter_mut()
            .find(|entry| entry.get_name().as_str() == name)
        {
            Some(entry) => *entry = template.clone(),
            None => self.upgrade_list.push(template.clone()),
        }
        self.upgrades.insert(name, template);
    }

    /// C++ `UpgradeCenter::parseUpgradeDefinition` (Upgrade.cpp:451-473):
    /// find by name, else `newUpgrade(name)`, then `initFromINI` overlays the
    /// C++ field table onto the existing template. Fields parsed before an
    /// error stay applied, as with C++'s in-place parse.
    pub fn parse_upgrade_definition(&mut self, ini: &mut INI) -> Result<(), INIError> {
        let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let existing = match self.find_upgrade(&name) {
            Some(existing) => existing,
            None => self.new_upgrade(AsciiString::from(name.as_str())),
        };
        let mut template = (*existing).clone();
        let result = template.parse_from_ini(ini);
        self.store_template(Arc::new(template));
        if let Err(err) = &result {
            log::warn!("Failed to parse upgrade '{name}': {err:?}");
        }
        result
    }

    /// C++ `UpgradeCenter::findUpgrade`.
    pub fn find_upgrade(&self, name: &str) -> Option<Arc<UpgradeTemplate>> {
        self.upgrades.get(name).cloned()
    }

    /// C++ `UpgradeCenter::findUpgradeByKey`.
    ///
    /// `key` is in the calling thread's NameKeyGenerator namespace (Rust's
    /// generator is thread-local; this center may be a clone built on another
    /// thread). Resolve the key back to its name there and look up by name —
    /// equal to C++ because `nameToKey` is a case-sensitive bijection.
    pub fn find_upgrade_by_key(&self, key: NameKeyType) -> Option<Arc<UpgradeTemplate>> {
        let name = NameKeyGenerator::key_to_name(key)?;
        self.find_upgrade(&name)
    }

    /// C++ `UpgradeCenter::findVeterancyUpgrade`.
    pub fn find_veterancy_upgrade(&self, level: &str) -> Option<Arc<UpgradeTemplate>> {
        self.find_upgrade(&format!("Upgrade_Veterancy_{level}"))
    }

    /// C++ `UpgradeCenter::firstUpgradeTemplate`.
    pub fn first_upgrade(&self) -> Option<Arc<UpgradeTemplate>> {
        self.upgrade_list.first().cloned()
    }

    /// All templates in C++ list order (newest first).
    pub fn get_all_upgrades(&self) -> &[Arc<UpgradeTemplate>] {
        &self.upgrade_list
    }

    /// C++ `UpgradeCenter::getUpgradeNames` (list order).
    pub fn get_upgrade_names(&self) -> Vec<AsciiString> {
        self.upgrade_list
            .iter()
            .map(|template| template.get_name().clone())
            .collect()
    }

    /// Mask of the named upgrade, or `None` when it is not defined (C++
    /// `findUpgrade` returning NULL).
    pub fn mask_for_name(&self, name: &str) -> Option<UpgradeMask> {
        self.find_upgrade(name).map(|template| template.get_mask())
    }

    /// Number of registered upgrades.
    pub fn count(&self) -> usize {
        self.upgrades.len()
    }

    /// C++ `Xfer::xferUpgradeMask` save half (Xfer.cpp): the names of every
    /// template whose mask is fully set, in upgrade-list order.
    pub fn upgrade_names_in_mask(&self, mask_bits: u128) -> Vec<String> {
        self.upgrade_list
            .iter()
            .filter(|template| {
                // A template past UPGRADE_MAX_COUNT owns no bit; never match it.
                let bits = template.get_mask().bits();
                bits != 0 && mask_bits & bits == bits
            })
            .map(|template| template.get_name().as_str().to_owned())
            .collect()
    }
}

/// The active world's UpgradeCenter (C++ single-pointer semantics: the one
/// live world's center). `None` resolves to the engine-lifetime center below,
/// so Upgrade.ini loads outside any world (engine boot, headless snippets,
/// tests without a world) keep working. GameLogic's EngineStores is the only
/// writer: it points this slot at the head world bundle's center.
static UPGRADE_CENTER_ACTIVE: RwLock<Option<Arc<RwLock<UpgradeCenter>>>> = RwLock::new(None);

/// Engine-lifetime UpgradeCenter (C++ `TheUpgradeCenter` created by
/// `GameEngine::init`, GameEngine.cpp:468). Every world snapshot-clones it.
static UPGRADE_CENTER_PROCESS_LIFETIME: LazyLock<Arc<RwLock<UpgradeCenter>>> =
    LazyLock::new(|| {
        let mut center = UpgradeCenter::new();
        // C++ UpgradeCenter::init runs before Upgrade.ini is parsed.
        center.init();
        Arc::new(RwLock::new(center))
    });

/// The engine-lifetime UpgradeCenter.
pub fn process_lifetime_upgrade_center() -> Arc<RwLock<UpgradeCenter>> {
    Arc::clone(&UPGRADE_CENTER_PROCESS_LIFETIME)
}

/// Initialize the engine-lifetime upgrade center (C++ engine boot order).
pub fn initialize_upgrade_center() {
    LazyLock::force(&UPGRADE_CENTER_PROCESS_LIFETIME);
}

/// The active UpgradeCenter (C++ `TheUpgradeCenter`): the installed world
/// bundle's center, or the engine-lifetime center when no world is active.
pub fn get_upgrade_center() -> Arc<RwLock<UpgradeCenter>> {
    let active = UPGRADE_CENTER_ACTIVE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    active
        .as_ref()
        .map(Arc::clone)
        .unwrap_or_else(|| Arc::clone(&UPGRADE_CENTER_PROCESS_LIFETIME))
}

/// Install a world bundle's UpgradeCenter and return the center it replaced.
pub fn install_upgrade_center(
    center: Arc<RwLock<UpgradeCenter>>,
) -> Option<Arc<RwLock<UpgradeCenter>>> {
    let mut active = UPGRADE_CENTER_ACTIVE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    active.replace(center)
}

/// Uninstall the active UpgradeCenter only if it is still `center` (a stale
/// world dropping after a newer world must not deactivate the newer world).
/// Returns `true` when the active slot was cleared.
pub fn uninstall_upgrade_center_if_current(center: &Arc<RwLock<UpgradeCenter>>) -> bool {
    let mut active = UPGRADE_CENTER_ACTIVE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match active.as_ref() {
        Some(current) if Arc::ptr_eq(current, center) => {
            *active = None;
            true
        }
        _ => false,
    }
}

/// Read-only access to the active center.
pub fn with_upgrade_center<R>(f: impl FnOnce(&UpgradeCenter) -> R) -> R {
    let center = get_upgrade_center();
    let center = center
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&center)
}

/// C++ `TheUpgradeCenter->findUpgrade(name)->getUpgradeMask()` on the active
/// center; `None` for names that are not defined.
pub fn upgrade_mask_for_name(name: &str) -> Option<UpgradeMask> {
    with_upgrade_center(|center| center.mask_for_name(name))
}
