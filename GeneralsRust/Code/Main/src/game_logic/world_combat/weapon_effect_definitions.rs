//! Receiving effective Weapon fire references and immutable OCL content.
//! CPP Weapon::fireWeaponTemplate889–949 selects exact source veterancy rows;
//! OCL definitions admit once and remain content across session resets.
use crate::game_logic::{GameLogic, VeterancyLevel};
use gamelogic::effects::ObjectCreationList;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::game_logic) struct ResolvedWeaponFireEffects {
    pub fire_fx: String,
    pub fire_ocl: String,
    pub play_fx_when_stealthed: bool,
}
#[derive(Debug, Default)]
pub(in crate::game_logic) struct WeaponEffectDefinitions {
    fire: HashMap<String, [ResolvedWeaponFireEffects; 4]>,
    ocls: HashMap<String, ObjectCreationList>,
    application_ocls: std::sync::Arc<HashMap<String, ObjectCreationList>>,
}
impl WeaponEffectDefinitions {
    pub(in crate::game_logic) fn fire(
        &self,
        name: &str,
        rank: VeterancyLevel,
    ) -> Option<ResolvedWeaponFireEffects> {
        let index = match rank {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        self.fire.get(name.trim()).map(|rows| rows[index].clone())
    }
    pub(in crate::game_logic) fn ocl(&self, name: &str) -> Option<ObjectCreationList> {
        self.ocls.get(name.trim()).cloned()
    }
}
impl GameLogic {
    /// Exact receiving named definition; absence never selects a global store.
    pub(crate) fn object_creation_list_definition(&self, name: &str) -> Option<ObjectCreationList> {
        self.weapon_effect_definitions.ocl(name)
    }

    /// One immutable application definition set is captured before match workers.
    /// Map/Solo replaces named lists on a private effective map, never this base.
    pub(crate) fn admit_ocl_application_sources(
        &mut self,
        sources: &[String],
    ) -> Result<(), String> {
        let mut definitions = HashMap::new();
        for source in sources {
            for (name, list) in
                gamelogic::object_creation_list::store::parse_object_creation_lists_from_ini(
                    source,
                )?
            {
                definitions.insert(name, list);
            }
        }
        self.weapon_effect_definitions.application_ocls = std::sync::Arc::new(definitions.clone());
        self.weapon_effect_definitions.ocls = definitions;
        Ok(())
    }
    pub(crate) fn ocl_application_definitions(
        &self,
    ) -> std::sync::Arc<HashMap<String, ObjectCreationList>> {
        std::sync::Arc::clone(&self.weapon_effect_definitions.application_ocls)
    }
    pub(crate) fn seed_ocl_application_definitions(
        &mut self,
        base: std::sync::Arc<HashMap<String, ObjectCreationList>>,
    ) {
        self.weapon_effect_definitions.ocls = (*base).clone();
        self.weapon_effect_definitions.application_ocls = base;
    }
    pub(crate) fn reset_ocl_map_definitions(&mut self) {
        self.weapon_effect_definitions.ocls =
            (*self.weapon_effect_definitions.application_ocls).clone();
    }
    pub(crate) fn apply_receiving_ocl_overrides(
        &mut self,
        sources: &[String],
    ) -> Result<(), String> {
        let mut candidate = self.weapon_effect_definitions.ocls.clone();
        for source in sources {
            for (name, list) in
                gamelogic::object_creation_list::store::parse_object_creation_lists_from_ini(
                    source,
                )?
            {
                candidate.insert(name, list);
            }
        }
        self.weapon_effect_definitions.ocls = candidate;
        Ok(())
    }
    pub(crate) fn admit_weapon_fire_definition(
        &mut self,
        name: &str,
        template: &gamelogic::weapon::WeaponTemplate,
    ) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("weapon fire definition name is blank".into());
        }
        if self.weapon_effect_definitions.fire.contains_key(name) {
            return Err(format!("weapon fire definition already admitted: {name}"));
        }
        let ranks = [
            gamelogic::common::VeterancyLevel::Regular,
            gamelogic::common::VeterancyLevel::Veteran,
            gamelogic::common::VeterancyLevel::Elite,
            gamelogic::common::VeterancyLevel::Heroic,
        ];
        let rows = std::array::from_fn(|index| ResolvedWeaponFireEffects {
            play_fx_when_stealthed: template.play_fx_when_stealthed,
            fire_fx: template
                .get_fire_fx(ranks[index])
                .map(|fx| fx.name().trim().to_owned())
                .unwrap_or_default(),
            // get_fire_ocl() performs ambient deferred resolution. Only the
            // retained immutable name is read here; the receiver admits OCL
            // content independently and never publishes a global store.
            fire_ocl: template
                .get_fire_ocl_name(ranks[index])
                .map(|name| name.trim().to_owned())
                .unwrap_or_default(),
        });
        self.weapon_effect_definitions
            .fire
            .insert(name.to_owned(), rows);
        Ok(())
    }
    pub(crate) fn admit_weapon_ocl_definition(
        &mut self,
        name: &str,
        ocl: ObjectCreationList,
    ) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("weapon OCL definition name is blank".into());
        }
        if self.weapon_effect_definitions.ocls.contains_key(name) {
            return Err(format!("weapon OCL definition already admitted: {name}"));
        }
        std::sync::Arc::make_mut(&mut self.weapon_effect_definitions.application_ocls)
            .insert(name.to_owned(), ocl.clone());
        self.weapon_effect_definitions
            .ocls
            .insert(name.to_owned(), ocl);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Team, ThingTemplate};
    fn native(prefix: &str) -> gamelogic::weapon::WeaponTemplate {
        let mut template = gamelogic::weapon::WeaponTemplate::new("SameWeapon".to_owned());
        for index in 0..4 {
            template.fire_fx[index] = Some(gamelogic::effects::FXList::new(&format!(
                "{prefix}FX{index}"
            )));
            template.fire_ocl_names[index] = Some(format!("{prefix}OCL{index}"));
        }
        template
    }
    #[test]
    fn same_id_receiving_effect_definitions_survive_peer_reset_without_override() {
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        for world in [&mut owner, &mut peer] {
            world
                .templates
                .insert("SameSource".into(), ThingTemplate::new("SameSource"));
        }
        let a = owner
            .create_object("SameSource", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let b = peer
            .create_object("SameSource", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        assert_eq!(a, b);
        let mut admitted = native("Owner");
        owner
            .admit_weapon_fire_definition("SameWeapon", &admitted)
            .unwrap();
        peer.admit_weapon_fire_definition("SameWeapon", &native("Peer"))
            .unwrap();
        owner
            .admit_weapon_ocl_definition("OwnerOCL0", ObjectCreationList::new())
            .unwrap();
        admitted.fire_ocl_names[0] = Some("AfterAdmissionMutation".into());
        peer.reset();
        for (index, rank) in [
            VeterancyLevel::Rookie,
            VeterancyLevel::Veteran,
            VeterancyLevel::Elite,
            VeterancyLevel::Heroic,
        ]
        .into_iter()
        .enumerate()
        {
            let effects = owner
                .weapon_effect_definitions
                .fire("SameWeapon", rank)
                .unwrap();
            assert_eq!(effects.fire_fx, format!("OwnerFX{index}"));
            assert_eq!(effects.fire_ocl, format!("OwnerOCL{index}"));
            assert_eq!(
                peer.weapon_effect_definitions
                    .fire("SameWeapon", rank)
                    .unwrap()
                    .fire_fx,
                format!("PeerFX{index}")
            );
        }
        owner.reset();
        assert!(owner.weapon_effect_definitions.ocl("OwnerOCL0").is_some());
        assert!(peer.weapon_effect_definitions.ocl("OwnerOCL0").is_none());
        assert!(
            owner
                .admit_weapon_fire_definition("SameWeapon", &native("Replacement"))
                .is_err()
        );
        assert!(
            owner
                .admit_weapon_ocl_definition("OwnerOCL0", ObjectCreationList::new())
                .is_err()
        );
        assert!(
            owner
                .admit_weapon_fire_definition(" ", &native("Invalid"))
                .is_err()
        );
        assert!(
            owner
                .admit_weapon_ocl_definition(" ", ObjectCreationList::new())
                .is_err()
        );
        assert_eq!(
            owner
                .weapon_effect_definitions
                .fire("SameWeapon", VeterancyLevel::Rookie)
                .unwrap()
                .fire_fx,
            "OwnerFX0"
        );
    }
}

#[cfg(test)]
#[path = "weapon_effect_definitions/ocl_content_tests.rs"]
mod ocl_content_tests;
