//! Observe this actual Main object's canonical fields at original Die/FX points.
use super::*;

impl Object {
    pub(crate) fn structure_topple_die_is_applicable(&self) -> bool {
        let default = gamelogic::object::die::DieMuxData::default();
        let mux = match &self.thing.template.structure_topple_definition {
            Some(Ok(definition)) => &definition.die_mux,
            Some(Err(_)) => return false,
            None => &default,
        };
        mux.is_die_applicable_for_death(
            crate::game_logic::host_fx_list_die::leftover_veterancy_from_host(
                self.experience.level,
            ),
            self.structure_topple_status_mask(),
            gamelogic::damage::DeathType::from_u32(self.status.death_type.ordinal() as u32),
        )
    }

    pub(crate) fn structure_topple_fx_is_allowed(&self) -> bool {
        let flags = match &self.thing.template.structure_topple_definition {
            Some(Ok(definition)) => definition.damage_fx_types,
            Some(Err(_)) => return false,
            None => gamelogic::damage::DamageTypeFlags::all_flags(),
        };
        // CPP ActiveBody.h:68 returns a record even before the first hit;
        // DamageInfoInput (Damage.h:243–247) initializes its type to EXPLOSION.
        // A missing timestamp/type in Main is not BodyModule's NULL record.
        // InactiveBody inherits that NULL getter and permits the FX instead.
        self.is_inactive_body()
            || gamelogic::damage::get_damage_type_flag(
                flags,
                self.last_damage_info_type
                    .unwrap_or(crate::game_logic::combat::DamageType::Explosive)
                    .to_store(),
            )
    }

    fn structure_topple_status_mask(&self) -> gamelogic::object::die::ObjectStatusMask {
        use gamelogic::common::ObjectStatusMaskType as Mask;
        // Unrepresented status bits retain their actual host bitset value.
        // Where Main owns a dedicated field, it is the observation authority;
        // stale bitset values must not override that field in either direction.
        let mut mask = Mask::from_bits_retain(self.object_status_mask_for_queries());
        for (bit, set) in [
            (Mask::DESTROYED, self.status.destroyed),
            (Mask::UNDER_CONSTRUCTION, self.status.under_construction),
            (Mask::UNSELECTABLE, self.status.unselectable),
            (Mask::NO_COLLISIONS, self.status.no_collisions),
            (Mask::AIRBORNE_TARGET, self.status.airborne_target),
            (Mask::PARACHUTING, self.status.parachuting),
            (Mask::REPULSOR, self.status.repulsor),
            (Mask::HIJACKED, self.status.hijacked),
            (Mask::IS_FIRING_WEAPON, self.status.is_firing_weapon),
            (Mask::BRAKING, self.is_braking),
            (Mask::STEALTHED, self.status.stealthed),
            (Mask::DETECTED, self.status.detected),
            (Mask::SOLD, self.status.sold),
            (Mask::RECONSTRUCTING, self.status.reconstructing),
            (Mask::MASKED, self.status.masked),
            (Mask::IS_ATTACKING, self.status.attacking),
            (Mask::IS_USING_ABILITY, self.status.using_ability),
            (Mask::IS_AIMING_WEAPON, self.status.is_aiming_weapon),
            (Mask::IGNORING_STEALTH, self.status.ignoring_stealth),
            (Mask::IS_CAR_BOMB, self.status.is_carbomb),
            (Mask::DECK_HEIGHT_OFFSET, self.status.deck_height_offset),
            (Mask::FAERIE_FIRE, self.status.faerie_fire),
            (Mask::DISGUISED, self.status.disguised),
            (Mask::DEPLOYED, self.status.deployed),
            (Mask::WET, self.status.wet),
        ] {
            mask.set(bit, set);
        }
        gamelogic::object::die::ObjectStatusMask::from_bits_retain(mask.bits())
    }
}
