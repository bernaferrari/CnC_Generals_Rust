//! Per-WeaponSet-slot barrel cursors.
//!
//! C++ owns `m_curBarrel` and `m_numShotsForCurBarrel` on each `Weapon`, not
//! on the Object.  Main's host `Weapon` intentionally remains a small shared
//! stat record, so retain the equivalent mutable state next to the Object's
//! three concrete WeaponSet slots instead of widening every host Weapon
//! literal.

use super::*;

pub const WEAPON_BARREL_SLOT_COUNT: usize = 3;

/// A raw v4 cursor awaiting a host-side, validated W3D barrel topology.
///
/// The cursor is deliberately bound to the exact active host WeaponSet slot
/// source and its authored cadence.  A later conditional set change must not
/// apply a save cursor that belonged to the old C++ `Weapon` instance.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingRestoredWeaponBarrelCursor {
    current_barrel: u8,
    shots_left_on_barrel: u32,
    source_weapon_name: Option<String>,
    shots_per_barrel: u32,
}

/// Mutable barrel cursor for one active C++ WeaponSet slot.
///
/// `shots_per_barrel` and `barrel_count` are re-derived from authored weapon
/// and draw data.  Save/load persists only `current_barrel` and
/// `shots_left_on_barrel`; the private source key lets a conditional set swap
/// reset the cursor exactly as C++ destroys and rebuilds its Weapon instances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponBarrelState {
    /// C++ `Weapon::m_curBarrel`.
    pub current_barrel: u8,
    /// C++ `WeaponTemplate::m_shotsPerBarrel`, clamped to at least one.
    pub shots_per_barrel: u32,
    /// Number of validated draw barrels for this slot, clamped to at least one.
    pub barrel_count: u8,
    /// C++ `Weapon::m_numShotsForCurBarrel`.
    pub shots_left_on_barrel: u32,
    /// Exact active Weapon.ini identity used to configure this state.  It is
    /// a runtime cache, never an authority for template selection.
    #[serde(skip)]
    source_weapon_name: Option<String>,
    /// A v4-restored cursor whose saved barrel index cannot be validated until
    /// a later authoritative draw-topology configuration supplies the actual
    /// barrel count.  Keeping it out of serde avoids reintroducing renderer
    /// cache state into Object persistence; the snapshot layer stores the two
    /// primitive cursor values directly.
    #[serde(skip)]
    pending_restored_cursor: Option<PendingRestoredWeaponBarrelCursor>,
}

impl Default for WeaponBarrelState {
    fn default() -> Self {
        Self::new(1, 1, None)
    }
}

impl WeaponBarrelState {
    pub fn new(
        shots_per_barrel: u32,
        barrel_count: u8,
        source_weapon_name: Option<String>,
    ) -> Self {
        let shots_per_barrel = shots_per_barrel.max(1);
        Self {
            current_barrel: 0,
            shots_per_barrel,
            barrel_count: barrel_count.max(1),
            shots_left_on_barrel: shots_per_barrel,
            source_weapon_name,
            pending_restored_cursor: None,
        }
    }

    fn advance_after_shot(&mut self) {
        let shots_per_barrel = self.shots_per_barrel.max(1);
        let barrel_count = self.barrel_count.max(1) as u32;
        if self.shots_left_on_barrel == 0 {
            self.shots_left_on_barrel = shots_per_barrel;
        }
        self.shots_left_on_barrel = self.shots_left_on_barrel.saturating_sub(1);
        if self.shots_left_on_barrel == 0 {
            self.current_barrel = ((self.current_barrel as u32 + 1) % barrel_count) as u8;
            self.shots_left_on_barrel = shots_per_barrel;
        }
    }

    fn normalize_runtime_cursor(&mut self) {
        self.shots_per_barrel = self.shots_per_barrel.max(1);
        self.barrel_count = self.barrel_count.max(1);
        self.current_barrel %= self.barrel_count;
        if self.shots_left_on_barrel == 0 || self.shots_left_on_barrel > self.shots_per_barrel {
            self.shots_left_on_barrel = self.shots_per_barrel;
        }
    }

    fn apply_restored_cursor(&mut self, current_barrel: u8, shots_left_on_barrel: u32) {
        self.current_barrel = current_barrel;
        self.shots_left_on_barrel = shots_left_on_barrel;
        self.normalize_runtime_cursor();
    }

    /// Once gameplay has accepted a shot before topology is available, the
    /// one-barrel live cursor is the only authoritative state left.  Dropping
    /// an unresolved saved multi-barrel cursor is fail-closed, but replaying
    /// it later would rewind an accepted post-load shot.
    fn discard_pending_cursor_for_live_use(&mut self) {
        self.pending_restored_cursor = None;
    }
}

impl Object {
    /// Return the exact authored Weapon.ini identity that backs one currently
    /// attached slot.  This intentionally does not use readiness's historical
    /// SECONDARY→PRIMARY fallback: a hand-authored secondary with no template
    /// must not inherit PRIMARY's barrel configuration.
    fn weapon_barrel_source_name_for_slot(&self, slot: u8) -> Option<&str> {
        match slot {
            0 if self.weapon_set_mine_clearing_detail
                && self.mine_clearing_primary_weapon.is_some() =>
            {
                self.thing
                    .template
                    .mine_clearing_primary_weapon_name
                    .as_deref()
            }
            0 => self.thing.template.primary_weapon_name.as_deref(),
            1 => self.thing.template.secondary_weapon_name.as_deref(),
            2 => self.thing.template.tertiary_weapon_name.as_deref(),
            _ => None,
        }
    }

    fn authored_shots_per_barrel_for_weapon_name(name: Option<&str>) -> u32 {
        use gamelogic::weapon::with_weapon_store;

        name.and_then(|name| {
            with_weapon_store(|store| {
                store
                    .find_weapon_template(name)
                    .map(|template| template.shots_per_barrel)
            })
            .ok()
            .flatten()
        })
        .and_then(|shots| u32::try_from(shots).ok())
        .unwrap_or(1)
        .max(1)
    }

    /// Synchronize one slot's cursor configuration with its exact active
    /// WeaponSet source.  C++ discards a Weapon instance when a conditional
    /// WeaponSet changes, which resets its barrel cursor; retain that behavior
    /// without resetting unrelated slots.
    fn ensure_weapon_barrel_state_for_slot(&mut self, slot: u8) -> Option<&mut WeaponBarrelState> {
        let index = usize::from(slot);
        if index >= WEAPON_BARREL_SLOT_COUNT {
            return None;
        }

        let source_weapon_name = self
            .weapon_barrel_source_name_for_slot(slot)
            .map(ToOwned::to_owned);
        let shots_per_barrel =
            Self::authored_shots_per_barrel_for_weapon_name(source_weapon_name.as_deref());
        let state = &mut self.weapon_barrel_states[index];
        let source_changed = state.source_weapon_name != source_weapon_name;
        let authored_config_changed =
            source_weapon_name.is_some() && state.shots_per_barrel != shots_per_barrel;
        if source_changed || authored_config_changed {
            *state = WeaponBarrelState::new(shots_per_barrel, 1, source_weapon_name);
        } else {
            state.normalize_runtime_cursor();
        }
        Some(state)
    }

    /// Barrel selected for a concrete, currently attached slot before its next
    /// accepted shot.  `None` rejects unavailable slots rather than inventing
    /// a visual/event identity for a missing weapon.
    pub fn fired_barrel_for_slot(&mut self, slot: u8) -> Option<u8> {
        self.weapon_slot(slot)?;
        let state = self.ensure_weapon_barrel_state_for_slot(slot)?;
        state.discard_pending_cursor_for_live_use();
        Some(state.current_barrel)
    }

    /// Advance only the concrete slot which just fired.
    pub fn advance_weapon_barrel_after_shot(&mut self, slot: u8) {
        if self.weapon_slot(slot).is_none() {
            return;
        }
        if let Some(state) = self.ensure_weapon_barrel_state_for_slot(slot) {
            state.discard_pending_cursor_for_live_use();
            state.advance_after_shot();
        }
    }

    /// Validated draw topology can later supply the actual barrel count for a
    /// slot.  Until then every authored weapon remains one-barrel, which is
    /// safer than guessing from a template or mesh name.
    pub fn set_weapon_barrel_count_for_slot(&mut self, slot: u8, barrel_count: u8) -> bool {
        let Some(state) = self.ensure_weapon_barrel_state_for_slot(slot) else {
            return false;
        };
        state.barrel_count = barrel_count.max(1);
        if let Some(pending) = state.pending_restored_cursor.take() {
            if pending.source_weapon_name == state.source_weapon_name
                && pending.shots_per_barrel == state.shots_per_barrel
            {
                state.apply_restored_cursor(pending.current_barrel, pending.shots_left_on_barrel);
            } else {
                // The selected host WeaponSet changed while topology was
                // unavailable.  C++ rebuilt this slot's Weapon, so its saved
                // mutable cursor cannot be transferred to the new instance.
                state.normalize_runtime_cursor();
            }
        } else {
            state.normalize_runtime_cursor();
        }
        true
    }

    /// Runtime state for snapshot capture and focused visual consumers.
    pub fn weapon_barrel_state_for_slot(&self, slot: u8) -> Option<&WeaponBarrelState> {
        self.weapon_barrel_states.get(usize::from(slot))
    }

    /// Restore the two mutable cursor values after authored configuration has
    /// been re-derived.  A saved multi-barrel index is held losslessly until
    /// the W3D topology layer has validated its barrel count; single-barrel
    /// slots apply it immediately.  Invalid values then fail closed to a
    /// fresh cursor instead of indexing a nonexistent draw barrel.
    pub fn restore_weapon_barrel_runtime_for_slot(
        &mut self,
        slot: u8,
        current_barrel: u8,
        shots_left_on_barrel: u32,
    ) -> bool {
        let Some(state) = self.ensure_weapon_barrel_state_for_slot(slot) else {
            return false;
        };
        if state.barrel_count > 1 {
            state.apply_restored_cursor(current_barrel, shots_left_on_barrel);
        } else if current_barrel == 0 {
            state.apply_restored_cursor(current_barrel, shots_left_on_barrel);
        } else {
            state.pending_restored_cursor = Some(PendingRestoredWeaponBarrelCursor {
                current_barrel,
                shots_left_on_barrel,
                source_weapon_name: state.source_weapon_name.clone(),
                shots_per_barrel: state.shots_per_barrel,
            });
        }
        true
    }

    /// Reset all slot cursors when a WeaponSet binding is replaced.  The
    /// caller first compares active source identities so ordinary flag writes
    /// that leave the set unchanged do not erase a live firing sequence.
    pub fn reset_weapon_barrel_states(&mut self) {
        self.weapon_barrel_states = std::array::from_fn(|_| WeaponBarrelState::default());
    }

    /// Reset only one slot when host code replaces that slot's concrete
    /// `Weapon` directly.  Most runtime condition changes retain their
    /// authored source identity and are handled by `ensure_*`; direct residual
    /// upgrades have no such identity, so they must say that a new instance
    /// was installed instead of inheriting an old barrel cursor.
    pub fn reset_weapon_barrel_state_for_slot(&mut self, slot: u8) {
        if let Some(state) = self.weapon_barrel_states.get_mut(usize::from(slot)) {
            *state = WeaponBarrelState::default();
        }
    }

    /// Install or remove a concrete standard WeaponSet slot as one atomic
    /// replacement.
    ///
    /// C++ `WeaponSet::updateWeaponSet` deletes the old `Weapon` and allocates
    /// a new one whenever its selected template set changes.  The narrow host
    /// residuals which model that transition must use this helper rather than
    /// assigning a field directly, so only the replaced slot loses its mutable
    /// barrel cursor.  Do not use it for a same-instance stat adjustment (for
    /// example a horde bonus): that must retain the current cursor.
    pub fn replace_weapon_set_slot(&mut self, slot: u8, weapon: Option<Weapon>) -> bool {
        match slot {
            0 => self.weapon = weapon,
            1 => self.secondary_weapon = weapon,
            2 => self.tertiary_weapon = weapon,
            _ => return false,
        }
        self.reset_weapon_barrel_state_for_slot(slot);
        true
    }

    pub(crate) fn active_weapon_barrel_source_identities(&self) -> [Option<String>; 3] {
        std::array::from_fn(|slot| {
            self.weapon_barrel_source_name_for_slot(slot as u8)
                .map(ToOwned::to_owned)
        })
    }

    pub(crate) fn reset_weapon_barrel_states_if_sources_changed(
        &mut self,
        previous_sources: [Option<String>; 3],
    ) {
        if previous_sources != self.active_weapon_barrel_source_identities() {
            self.reset_weapon_barrel_states();
        }
    }
}
