//! Original immediate Open/Transport death, evacuation, and literal onDelete phases.
use super::*;
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::modules::OwnedContainDeathPolicy;

impl BorrowedAiWorld<'_> {
    fn validate_containment_binding_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let object = owner.read().map_err(|_| "contain binding owner poisoned")?;
        let Some(actual) = object.get_contain() else {
            return Ok(());
        };
        let handles = object.behavior_modules();
        drop(object);
        let mut matched = 0;
        for handle in handles {
            let contain = handle
                .try_with_module(|module| {
                    crate::contain_module_overrides::contain_handle_for_module(module)
                })
                .ok_or("contain binding module borrowed")?;
            if let Some(contain) = contain {
                if !Arc::ptr_eq(&actual, &contain) {
                    return Err("competing contain binding owners".into());
                }
                matched += 1;
            }
        }
        if matched != 1 {
            return Err("contain lifecycle requires one actual authored binding".into());
        }
        Ok(())
    }

    fn physical_containment_members_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<Vec<ObjectID>, String> {
        let object = owner
            .read()
            .map_err(|_| "physical contain owner poisoned")?;
        let actual = self
            .object(object.get_id())
            .ok_or("physical contain owner absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("foreign physical contain owner".into());
        }
        let contain = object.get_contain();
        drop(object);
        let Some(contain) = contain else {
            return Ok(Vec::new());
        };
        let contain = contain
            .try_lock()
            .map_err(|_| "physical contain module borrowed")?;
        if !contain.supports_owned_lifecycle() {
            return Err("physical contain lifecycle callback not supplied".into());
        }
        contain
            .owned_physical_members()
            .ok_or("physical contain roster callback not supplied".into())
    }

    /// Capture only this item's successor before invoking its original callback.
    /// Future Objects are resolved when visited, never retained across callbacks.
    fn physical_containment_successor_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
        current: ObjectID,
    ) -> Result<Option<ObjectID>, String> {
        let members = self.physical_containment_members_owned(owner)?;
        let index = members
            .iter()
            .position(|id| *id == current)
            .ok_or("physical contain cursor retired")?;
        Ok(members.get(index + 1).copied())
    }

    pub(crate) fn order_contained_exit_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        source: crate::ai::CommandSourceType,
        instantly: bool,
    ) -> Result<(), String> {
        let mut current = self
            .physical_containment_members_owned(owner)?
            .first()
            .copied();
        while let Some(id) = current {
            current = self.physical_containment_successor_owned(owner, id)?;
            let rider = self.object(id).ok_or("ordered exit rider absent")?;
            let has_ai = rider
                .read()
                .map_err(|_| "ordered exit rider poisoned")?
                .get_ai_update_interface()
                .is_some();
            if has_ai {
                self.execute_exit_command(id, instantly, source)?;
            }
        }
        Ok(())
    }

    fn containment_death_policy_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
        damage: &DamageInfo,
    ) -> Result<Option<OwnedContainDeathPolicy>, String> {
        self.validate_containment_binding_owned(owner)?;
        let object = owner.read().map_err(|_| "contain death owner poisoned")?;
        let Some(contain) = object.get_contain() else {
            return Ok(None);
        };
        let contain = contain
            .try_lock()
            .map_err(|_| "contain death module borrowed")?;
        if !contain.supports_owned_lifecycle() {
            return Err("contain death callback family not supplied".into());
        }
        let policy = contain
            .owned_death_policy(&object, damage)
            .ok_or("contain death policy not supplied")?;
        if !policy.damage_percent.is_finite() {
            return Err("contain damage percentage is nonfinite".into());
        }
        Ok(Some(policy))
    }

    fn blocked_rider_free_to_exit_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
        rider: &Arc<RwLock<Object>>,
    ) -> Result<bool, String> {
        let object = owner.read().map_err(|_| "blocked exit owner poisoned")?;
        let (id, layer, position, owner_ai) = (
            object.get_id(),
            object.get_layer(),
            *object.get_position(),
            object.get_ai_update_interface(),
        );
        drop(object);
        let passenger = rider.read().map_err(|_| "blocked exit rider poisoned")?;
        let rider_ai = passenger.get_ai_update_interface();
        let mut airborne = false;
        if let Some(owner_ai) = owner_ai {
            let owner_ai = owner_ai
                .try_lock()
                .map_err(|_| "blocked exit owner AI borrowed")?;
            if owner_ai.get_ai_free_to_exit_with_world(&passenger, self)?
                != crate::object::production::AIFreeToExitType::FreeToExit
            {
                return Ok(false);
            }
            airborne = crate::object::contain::transport_contain::selected_locomotor_is_airborne(
                &*owner_ai,
            );
        }
        drop(passenger);
        if airborne {
            return Ok(true);
        }
        let Some(rider_ai) = rider_ai else {
            return Ok(false);
        };
        let rider_ai = rider_ai
            .try_lock()
            .map_err(|_| "blocked exit rider AI borrowed")?;
        let mut valid = false;
        rider_ai.with_cur_locomotor(&mut |loco| {
            valid = self
                .services
                .pathfinder
                .valid_movement_terrain_for_locomotor(
                    crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32),
                    loco,
                    &position,
                );
        });
        let _ = id;
        Ok(valid)
    }

    fn contained_damage_owned(
        &self,
        rider: &Arc<RwLock<Object>>,
        source: ObjectID,
        policy: OwnedContainDeathPolicy,
    ) -> Result<DamageInfo, String> {
        let body = rider
            .read()
            .map_err(|_| "contained damage rider poisoned")?
            .get_body_module()
            .ok_or("contained damage body absent")?;
        let amount = body
            .try_lock()
            .map_err(|_| "contained damage body borrowed")?
            .get_max_health()
            * policy.damage_percent;
        if !amount.is_finite() {
            return Err("contained damage amount is nonfinite".into());
        }
        Ok(DamageInfo::with_simple(
            amount,
            source,
            DamageType::Unresistable,
            if policy.burned {
                DeathType::Burned
            } else {
                DeathType::Normal
            },
        ))
    }

    pub(super) fn validate_damage_containment_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let (contain, parent) = {
            let owner = owner.read().map_err(|_| "damage contain owner poisoned")?;
            (owner.get_contain(), owner.get_contained_by())
        };
        if let Some(contain) = contain {
            self.validate_containment_binding_owned(owner)?;
            if !contain
                .try_lock()
                .map_err(|_| "damage contain module borrowed")?
                .supports_owned_lifecycle()
            {
                return Err("damage containment callbacks not supplied".into());
            }
            self.validate_containment_transform_owned(owner)?;
        }
        if let Some(parent) = parent {
            let parent = self.object(parent).ok_or("damage contained owner absent")?;
            self.validate_remove_operation(owner, &parent, false)?;
        }
        Ok(())
    }

    pub(super) fn validate_containment_death_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
        damage: &DamageInfo,
    ) -> Result<(), String> {
        let tunnel_contain = {
            owner
                .read()
                .map_err(|_| "tunnel DIE owner poisoned")?
                .get_contain()
        };
        if let Some(contain) = tunnel_contain {
            let tunnel_applicable = {
                let object = owner.read().map_err(|_| "tunnel DIE owner poisoned")?;
                contain
                    .try_lock()
                    .map_err(|_| "tunnel DIE module borrowed")?
                    .owned_tunnel_die_applicable(&object, damage)
            };
            if let Some(applicable) = tunnel_applicable {
                if applicable {
                    self.validate_tunnel_retirement_owned(owner)?;
                }
                return Ok(());
            }
        }
        let Some(policy) = self.containment_death_policy_owned(owner, damage)? else {
            return Ok(());
        };
        if !policy.applicable {
            return Ok(());
        }
        let id = owner
            .read()
            .map_err(|_| "contain death owner poisoned")?
            .get_id();
        let mut current = self
            .physical_containment_members_owned(owner)?
            .first()
            .copied();
        while let Some(rider_id) = current {
            current = self.physical_containment_successor_owned(owner, rider_id)?;
            let rider = self
                .object(rider_id)
                .ok_or("contained death rider absent")?;
            self.validate_remove_operation(&rider, owner, false)?;
            if policy.damage_percent > 0.0
                && !rider
                    .read()
                    .map_err(|_| "contained death rider poisoned")?
                    .is_effectively_dead()
            {
                let damage = self.contained_damage_owned(&rider, id, policy)?;
                self.validate_damage_owned(&rider, &damage)?;
            }
            if policy.destroy_blocked.is_some()
                && !self.blocked_rider_free_to_exit_owned(owner, &rider)?
            {
                if policy.destroy_blocked == Some(true) {
                    self.validate_owned_destroy(rider_id)?;
                } else if !rider
                    .read()
                    .map_err(|_| "blocked death rider poisoned")?
                    .is_effectively_dead()
                {
                    self.validate_damage_owned(
                        &rider,
                        &Self::physics_resting_kill_damage(&rider)?,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn finish_containment_death_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        damage: &DamageInfo,
    ) -> Result<(), String> {
        let tunnel_contain = {
            owner
                .read()
                .map_err(|_| "tunnel DIE owner poisoned")?
                .get_contain()
        };
        if let Some(contain) = tunnel_contain {
            let applicable = {
                let object = owner.read().map_err(|_| "tunnel DIE owner poisoned")?;
                contain
                    .try_lock()
                    .map_err(|_| "tunnel DIE module borrowed")?
                    .owned_tunnel_die_applicable(&object, damage)
            };
            if let Some(applicable) = applicable {
                if applicable {
                    self.retire_tunnel_owned(owner)?;
                }
                return Ok(());
            }
        }
        let Some(policy) = self.containment_death_policy_owned(owner, damage)? else {
            return Ok(());
        };
        if !policy.applicable {
            return Ok(());
        }
        let source = owner
            .read()
            .map_err(|_| "contain death owner poisoned")?
            .get_id();
        if policy.damage_percent > 0.0 {
            let mut current = self
                .physical_containment_members_owned(owner)?
                .first()
                .copied();
            while let Some(id) = current {
                current = self.physical_containment_successor_owned(owner, id)?;
                let rider = self.object(id).ok_or("contained damage rider absent")?;
                if !rider
                    .read()
                    .map_err(|_| "contained damage rider poisoned")?
                    .is_effectively_dead()
                {
                    let mut damage = self.contained_damage_owned(&rider, source, policy)?;
                    self.attempt_damage_owned(&rider, &mut damage)?;
                }
                if policy.damage_percent == 1.0
                    && !rider
                        .read()
                        .map_err(|_| "contained damage rider poisoned")?
                        .is_effectively_dead()
                {
                    let mut kill = Self::physics_resting_kill_damage(&rider)?;
                    self.attempt_damage_owned(&rider, &mut kill)?;
                }
            }
        }
        if let Some(destroy) = policy.destroy_blocked {
            let mut current = self
                .physical_containment_members_owned(owner)?
                .first()
                .copied();
            while let Some(id) = current {
                current = self.physical_containment_successor_owned(owner, id)?;
                let rider = self.object(id).ok_or("blocked death rider absent")?;
                if !self.blocked_rider_free_to_exit_owned(owner, &rider)? {
                    if destroy {
                        self.destroy_object_owned(id)?;
                    } else if !rider
                        .read()
                        .map_err(|_| "blocked death rider poisoned")?
                        .is_effectively_dead()
                    {
                        let mut kill = Self::physics_resting_kill_damage(&rider)?;
                        self.attempt_damage_owned(&rider, &mut kill)?;
                    }
                }
            }
        }
        self.remove_all_physical_contained_owned(owner)
    }

    pub(crate) fn remove_all_physical_contained_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let owner_id = owner
            .read()
            .map_err(|_| "evacuation owner poisoned")?
            .get_id();
        let mut current = self
            .physical_containment_members_owned(owner)?
            .first()
            .copied();
        while let Some(id) = current {
            current = self.physical_containment_successor_owned(owner, id)?;
            self.remove_contained(id, owner_id, false)?;
        }
        Ok(())
    }

    pub(super) fn validate_containment_delete_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        if self.validate_tunnel_retirement_owned(owner)? {
            return Ok(());
        }
        if owner
            .read()
            .map_err(|_| "contain delete owner poisoned")?
            .get_contain()
            .is_none()
        {
            return Ok(());
        }
        self.validate_containment_binding_owned(owner)?;
        self.validate_containment_transform_owned(owner)?;
        let mut current = self
            .physical_containment_members_owned(owner)?
            .first()
            .copied();
        while let Some(id) = current {
            current = self.physical_containment_successor_owned(owner, id)?;
            self.validate_owned_destroy(id)?;
        }
        Ok(())
    }

    pub(super) fn finish_containment_delete_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        handle: &crate::object::BehaviorModuleHandle,
    ) -> Result<bool, String> {
        let contain = handle.with_module(|module| {
            crate::contain_module_overrides::contain_handle_for_module(module)
        });
        let Some(contain) = contain else {
            return Ok(false);
        };
        let actual = owner
            .read()
            .map_err(|_| "contain delete owner poisoned")?
            .get_contain()
            .ok_or("contain delete interface absent")?;
        if !Arc::ptr_eq(&contain, &actual) {
            return Err("contain delete handle/interface differ".into());
        }
        if self.retire_tunnel_owned(owner)? {
            return Ok(true);
        }
        let mut current = self
            .physical_containment_members_owned(owner)?
            .first()
            .copied();
        while let Some(id) = current {
            current = self.physical_containment_successor_owned(owner, id)?;
            self.destroy_object_owned(id)?;
        }
        Ok(true)
    }
}
