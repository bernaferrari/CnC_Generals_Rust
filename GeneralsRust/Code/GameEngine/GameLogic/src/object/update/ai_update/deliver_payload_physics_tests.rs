//! Exercise the synchronous weapon/death consumers of canonical physics.

use super::*;
use crate::contain_module_overrides::ActiveBehaviorModule;
use crate::modules::ContainModuleInterface;
use crate::object::behavior::physics_update::{PhysicsBehaviorModuleData, PhysicsBehaviorUpdate};
use crate::object::{Object, PhysicsInterfaceHandle};
use crate::weapon::{WeaponSlotType, WeaponStatus, WeaponTemplate, WeaponTemplateSet};
use game_engine::common::thing::module::BaseModuleData;
use std::borrow::Cow;
use std::sync::{Mutex, RwLock};

#[derive(Debug)]
struct DeliveryAircraftTemplate {
    base: crate::common::DefaultThingTemplate,
    weapons: [game_engine::thing::thing_template::WeaponTemplateSet; 1],
}

impl DeliveryAircraftTemplate {
    fn new() -> Self {
        let mut weapons = game_engine::thing::thing_template::WeaponTemplateSet::new();
        weapons.set_weapon_template_name(0, Some("DeliveryStrafingWeapon".into()));
        Self {
            base: crate::common::DefaultThingTemplate::new("DeliveryAircraft".into()),
            weapons: [weapons],
        }
    }
}

impl ThingTemplate for DeliveryAircraftTemplate {
    fn get_name(&self) -> &AsciiString {
        self.base.get_name()
    }
    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        self.base.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> Real {
        self.base.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> Real {
        self.base.calc_shroud_clearing_range()
    }
    fn is_kind_of(&self, kind: crate::common::KindOf) -> bool {
        self.base.is_kind_of(kind)
    }
    fn weapon_template_sets(&self) -> &[game_engine::thing::thing_template::WeaponTemplateSet] {
        &self.weapons
    }
}

struct RegisteredAircraft {
    id: ObjectID,
    owner: Arc<RwLock<Object>>,
    physics: PhysicsInterfaceHandle,
}

impl RegisteredAircraft {
    fn new(id: ObjectID) -> Self {
        // CPP Object.cpp:364–377 installs the real FiringTracker ctor helper
        // only when the authored template can possibly have a weapon.
        let mut object =
            Object::new_test_from_template(id, 100.0, Arc::new(DeliveryAircraftTemplate::new()));
        if let Some(terrain) = TheTerrainLogic::get() {
            let extent = terrain.get_extent_including_border();
            object
                .set_position(&((extent.lo + extent.hi) * 0.5))
                .unwrap();
        }
        let owner = Arc::new(RwLock::new(object));
        let data = Arc::new(PhysicsBehaviorModuleData::default());
        let behavior_data: Arc<dyn crate::common::ModuleData> = data.clone();
        let module_data: Arc<dyn ModuleData> = data;
        let behavior = PhysicsBehaviorUpdate::new(owner.clone(), behavior_data).unwrap();
        let module = ActiveBehaviorModule::new("PhysicsBehavior", module_data.clone(), behavior);
        let physics = {
            let mut object = owner.write().unwrap();
            object.install_update_module("PhysicsBehavior", Box::new(module), module_data);
            let entry = object
                .module_by_name(&AsciiString::from("PhysicsBehavior"))
                .unwrap();
            let physics = PhysicsInterfaceHandle::from_module(entry);
            object.set_physics(Some(physics.clone()));
            physics
        };
        crate::object::registry::OBJECT_REGISTRY.register_object(id, &owner);
        Self { id, owner, physics }
    }
}

impl Drop for RegisteredAircraft {
    fn drop(&mut self) {
        // A regression assertion runs inside the real dispatch's storage
        // guards. On RED, release poisoned test modules before Object::Drop
        // invokes onDelete; otherwise a second panic would hide the diagnosis.
        if std::thread::panicking() {
            let mut object = self
                .owner
                .write()
                .unwrap_or_else(|error| error.into_inner());
            object.physics = None;
            object.behaviors.clear();
            object.update_module_handles.clear();
            object.modules.clear();
        }
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.id);
    }
}

/// This entry is visited by Object's actual UPDATE-module notification scan.
/// It does not dispatch or skip physics: a failed nonblocking borrow diagnoses
/// the old retained lease before that scan would block on the next entry.
struct FiringScanProbe {
    physics: PhysicsInterfaceHandle,
    visits: Arc<Mutex<usize>>,
    data: Arc<BaseModuleData>,
}

impl Snapshotable for FiringScanProbe {
    fn crc(&self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Module for FiringScanProbe {
    fn as_any(&self) -> &dyn Any {
        assert!(
            self.physics.try_access().is_ok(),
            "delivery firing must release canonical physics before the UPDATE scan"
        );
        *self.visits.lock().unwrap() += 1;
        self
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

#[derive(Debug)]
struct IdleAi;

impl AIUpdateInterface for IdleAi {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _: &Coord3D) -> Result<(), String> {
        Ok(())
    }
}

/// Invoked by the actual authored FireOCL dispatch before the firing scan.
/// The default nugget object overload forwards the real borrowed primary.
struct FiringOclProbe {
    physics: PhysicsInterfaceHandle,
    visits: Arc<Mutex<usize>>,
    creations: Arc<Mutex<usize>>,
}

impl crate::object_creation_list::ObjectCreationNugget for FiringOclProbe {
    fn create_with_angle(
        &self,
        _: &crate::object_creation_list::CreationContext<'_>,
        primary_object: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        _: Real,
        lifetime_frames: u32,
    ) -> crate::object_creation_list::CreationResult {
        let owner = primary_object.expect("FireOCL must retain the real source Object");
        assert_eq!(*primary, *owner.get_position());
        assert_eq!(*secondary, *primary);
        assert_eq!(lifetime_frames, 0);
        assert!(
            self.physics.try_access().is_ok(),
            "FireOCL must release canonical physics"
        );
        assert_eq!(
            *self.visits.lock().unwrap(),
            0,
            "CPP FireOCL precedes firing tracker dispatch"
        );
        *self.creations.lock().unwrap() += 1;
        None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn delivery_strafe_releases_physics_before_real_firing_tracker_scan() {
    let _lock = crate::test_sync::lock();
    let aircraft = RegisteredAircraft::new(93_541);
    aircraft
        .physics
        .access()
        .unwrap()
        .set_velocity(&Coord3D::new(10.0, 0.0, 0.0));
    let visits = Arc::new(Mutex::new(0));
    let creations = Arc::new(Mutex::new(0));
    {
        let mut object = aircraft.owner.write().unwrap();
        let data = Arc::new(BaseModuleData::new());
        object.install_update_module(
            "FiringScanProbe",
            Box::new(FiringScanProbe {
                physics: aircraft.physics.clone(),
                visits: visits.clone(),
                data: data.clone(),
            }),
            data,
        );
        // Authored ordering is probe then actual PhysicsBehavior. The scan
        // continues through both before reaching the owned ctor tracker.
        let last = object.update_module_handles.len() - 1;
        object.update_module_handles.swap(0, last);
        let mut template = WeaponTemplate::new("DeliveryStrafingWeapon".into());
        template.clip_size = 2;
        template.reload_type = crate::weapon::WeaponReloadType::NoReload;
        template.attack_range = 1_000.0;
        let mut fire_ocl = crate::object_creation_list::ObjectCreationList::new();
        fire_ocl.add_nugget(Arc::new(FiringOclProbe {
            physics: aircraft.physics.clone(),
            visits: visits.clone(),
            creations: creations.clone(),
        }));
        template.fire_ocl[0] = Some(Arc::new(fire_ocl));
        let mut set = WeaponTemplateSet::new();
        set.set_weapon_template(WeaponSlotType::Primary, Arc::new(template));
        object.weapon_set.add_weapon_template_set(set);
        object.refresh_weapon_set().unwrap();
        let weapon = object
            .get_weapon_in_slot_mut(WeaponSlotType::Primary)
            .unwrap();
        weapon.set_clip_percent_full(1.0, true);
        weapon.set_max_shot_count(10);
        weapon.set_status(WeaponStatus::ReadyToFire);
        assert_eq!(
            object.firing_tracker.as_ref().unwrap().xfer_test_state().0,
            0
        );
    }
    // CPP DeliverPayloadAIUpdate.cpp:187–217: velocity copy, then immediate
    // temporary weapon lock/fire, then strafe FX. This test executes firing;
    // the process-wide FX manager is deliberately outside this fixture.
    let mut delivery =
        DeliverPayloadAIUpdate::new(DeliverPayloadAIUpdateModuleData::default(), aircraft.id);
    delivery.target_pos =
        *aircraft.owner.read().unwrap().get_position() + Coord3D::new(50.0, 0.0, 0.0);
    delivery.dive_state = DiveState::Diving;
    delivery.data.dive_start_distance = 100.0;
    delivery.data.dive_end_distance = 10.0;
    delivery.data.strafing_weapon_slot = Some(crate::common::WeaponSlotType::Primary);
    delivery.update(&mut IdleAi).unwrap();

    assert!(
        *visits.lock().unwrap() > 0,
        "real UPDATE notification scan ran"
    );
    assert_eq!(
        *creations.lock().unwrap(),
        1,
        "real FireOCL dispatch ran once"
    );
    let object = aircraft.owner.read().unwrap();
    let weapon = object.get_weapon_in_slot(WeaponSlotType::Primary).unwrap();
    assert_eq!(weapon.get_max_shot_count(), 9, "one real round was fired");
    assert_eq!(
        object.firing_tracker.as_ref().unwrap().xfer_test_state().0,
        1,
        "the scan completed and notified the actual owned firing tracker"
    );
    assert!(object.is_cur_weapon_locked());
    assert_eq!(delivery.dive_state, DiveState::Diving);
    assert!(aircraft.physics.try_access().is_ok());
}

#[derive(Debug)]
struct DeathReceiver {
    physics: PhysicsInterfaceHandle,
    deaths: Arc<Mutex<usize>>,
}

impl ContainModuleInterface for DeathReceiver {
    fn can_contain(&self, _: ObjectID) -> bool {
        false
    }
    fn contain_object(&mut self, _: ObjectID) -> Result<(), String> {
        Ok(())
    }
    fn release_object(&mut self, _: ObjectID) -> Result<(), String> {
        Ok(())
    }
    fn get_contained_objects(&self) -> Cow<'_, [ObjectID]> {
        Cow::Borrowed(&[])
    }
    fn get_contained_count(&self) -> usize {
        0
    }
    fn get_max_capacity(&self) -> usize {
        0
    }
    fn on_die_with_owner(
        &mut self,
        owner: &Object,
        damage: Option<&crate::damage::DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        assert!(
            owner.is_effectively_dead(),
            "death callback runs after lethal transition"
        );
        assert!(damage.unwrap().input.kill);
        assert!(
            self.physics.try_access().is_ok(),
            "delivery death must release canonical physics before synchronous callbacks"
        );
        *self.deaths.lock().unwrap() += 1;
        Ok(())
    }
}

#[test]
fn delivery_head_off_map_releases_physics_before_real_death_callback() {
    let _lock = crate::test_sync::lock();
    let aircraft = RegisteredAircraft::new(93_542);
    aircraft.physics.access().unwrap().set_turning(1);
    let deaths = Arc::new(Mutex::new(0));
    aircraft
        .owner
        .write()
        .unwrap()
        .set_contain(Some(Arc::new(Mutex::new(DeathReceiver {
            physics: aircraft.physics.clone(),
            deaths: deaths.clone(),
        }))));
    // CPP DeliverPayloadAIUpdate.cpp:1182–1191: nonzero turning and an
    // opposing delivery heading kill immediately, before returning CONTINUE.
    let mut delivery =
        DeliverPayloadAIUpdate::new(DeliverPayloadAIUpdateModuleData::default(), aircraft.id);
    delivery.facing_direction_upon_delivery = Coord3D::new(-1.0, 0.0, 0.0);
    assert_eq!(delivery.update_head_off_map(), StateReturnType::Continue);
    assert!(aircraft.owner.read().unwrap().is_effectively_dead());
    assert_eq!(
        *deaths.lock().unwrap(),
        1,
        "real Object::on_die callback ran synchronously"
    );
    assert!(aircraft.physics.try_access().is_ok());
}

#[test]
fn held_weapon_range_preserves_geometry_minimum_self_target_and_source_identity() {
    let _lock = crate::test_sync::lock();
    let id = 93_551;
    let retired = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    let current = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    retired.write().unwrap().geometry_info.bounds.min = Coord3D::new(-10.0, 0.0, -1.0);
    retired.write().unwrap().geometry_info.bounds.max = Coord3D::new(10.0, 0.0, 1.0);
    current.write().unwrap().geometry_info.position = Coord3D::new(500.0, 0.0, 0.0);
    crate::object::registry::OBJECT_REGISTRY.register_object(id, &current);

    let mut template = WeaponTemplate::new("HeldSourceRange".into());
    template.attack_range = 102.5; // CPP PATHFIND_CELL_SIZE_F/4 gives range 100.
    template.minimum_attack_range = 12.5; // CPP rationalizes min by2.5 too: effective10.
    let mut weapon = crate::weapon::Weapon::new(Arc::new(template), WeaponSlotType::Primary);
    let retired_guard = retired.write().unwrap();
    let current_guard = current.write().unwrap();
    weapon.set_caller_held_source(
        &retired_guard,
        crate::weapon::WeaponBonusConditionFlags::new(),
    );
    assert!(weapon.caller_source(id + 1).is_none());
    // Both objects use the same externally meaningful ID. The supplied C++
    // source pointer wins over the registry's newer allocation, even locked.
    assert!(weapon.is_within_attack_range(id, None, Some(&Coord3D::new(20.0, 0.0, 0.0))));
    assert!(!weapon.is_within_attack_range(id, None, Some(&Coord3D::new(19.0, 0.0, 0.0))));
    assert!(!weapon.is_too_close(id, None, Some(&Coord3D::new(20.0, 0.0, 0.0))));
    assert!(weapon.is_too_close(id, None, Some(&Coord3D::new(19.0, 0.0, 0.0))));
    assert!(weapon.is_within_attack_range(id, None, Some(&Coord3D::new(110.0, 0.0, 0.0))));
    assert!(!weapon.is_within_attack_range(id, None, Some(&Coord3D::new(110.25, 0.0, 0.0))));
    assert!(weapon.is_too_close(id, Some(id), None));
    assert!(!weapon.is_within_attack_range(id, Some(id), None));
    Arc::make_mut(&mut weapon.template).minimum_attack_range = 0.0;
    assert!(!weapon.is_too_close(id, Some(id), None));
    assert!(weapon.is_within_attack_range(id, Some(id), None));
    weapon.clear_caller_held_source();
    assert!(weapon.caller_source(id).is_none());
    drop(current_guard);
    assert!(
        !weapon.is_within_attack_range(id, None, Some(&Coord3D::new(20.0, 0.0, 0.0))),
        "cleared input must not keep the retired source geometry"
    );
    drop(retired_guard);
    crate::object::registry::OBJECT_REGISTRY.unregister_object(id);
}

#[test]
fn held_weapon_range_combines_borrowed_and_extra_bonus_flags() {
    let _lock = crate::test_sync::lock();
    use crate::weapon::{
        WeaponBonus, WeaponBonusConditionFlags, WeaponBonusConditionType, WeaponBonusField,
        WeaponBonusSet,
    };
    let owner = Arc::new(RwLock::new(Object::new_test(93_552, 100.0)));
    let owner = owner.write().unwrap();
    let mut template = WeaponTemplate::new("HeldSourceRangeBonus".into());
    template.attack_range = 102.5;
    let mut bonuses = WeaponBonusSet::new();
    let mut veteran = WeaponBonus::new();
    veteran.set_field(WeaponBonusField::Range, 2.0);
    bonuses.set_bonus(WeaponBonusConditionType::Veteran, veteran);
    let mut garrisoned = WeaponBonus::new();
    garrisoned.set_field(WeaponBonusField::Range, 1.5);
    bonuses.set_bonus(WeaponBonusConditionType::Garrisoned, garrisoned);
    template.extra_bonus = Some(bonuses);
    let mut weapon = crate::weapon::Weapon::new(Arc::new(template), WeaponSlotType::Primary);
    let mut source_flags = WeaponBonusConditionFlags::new();
    source_flags.set(WeaponBonusConditionType::Veteran);
    weapon.set_caller_held_source(&owner, source_flags);
    assert!(weapon.is_within_attack_range(
        owner.get_id(),
        None,
        Some(&Coord3D::new(200.0, 0.0, 0.0))
    ));
    assert!(!weapon.is_within_attack_range(
        owner.get_id(),
        None,
        Some(&Coord3D::new(203.0, 0.0, 0.0))
    ));
    let mut extra_flags = WeaponBonusConditionFlags::new();
    extra_flags.set(WeaponBonusConditionType::Garrisoned);
    assert_eq!(
        weapon
            .compute_bonus(owner.get_id(), extra_flags)
            .get_field(WeaponBonusField::Range),
        2.5,
        "CPP ORs flags and adds multiplier deltas, rather than multiplying them"
    );
    weapon.clear_caller_held_source();
}

struct HealthAtFireOcl {
    health: Arc<Mutex<Vec<f32>>>,
}

impl crate::object_creation_list::ObjectCreationNugget for HealthAtFireOcl {
    fn create_with_angle(
        &self,
        _: &crate::object_creation_list::CreationContext<'_>,
        primary: Option<&Object>,
        _: &Coord3D,
        _: &Coord3D,
        _: Real,
        _: u32,
    ) -> crate::object_creation_list::CreationResult {
        self.health
            .lock()
            .unwrap()
            .push(primary.unwrap().get_health());
        None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn held_weapon_damage_preserves_fire_ocl_before_owned_self_damage() {
    let _lock = crate::test_sync::lock();
    let aircraft = RegisteredAircraft::new(93_562);
    let health_at_ocl = Arc::new(Mutex::new(Vec::new()));
    let mut fire_ocl = crate::object_creation_list::ObjectCreationList::new();
    fire_ocl.add_nugget(Arc::new(HealthAtFireOcl {
        health: health_at_ocl.clone(),
    }));
    let mut template = WeaponTemplate::new("DeliveryStrafingWeapon".into());
    template.clip_size = 2;
    template.reload_type = crate::weapon::WeaponReloadType::NoReload;
    template.attack_range = 1_000.0;
    template.primary_damage = 25.0;
    template.primary_damage_radius = 10.0;
    template.affects_mask = crate::weapon::WeaponAffectsMask::new(
        crate::weapon::WeaponAffectsMask::SELF | crate::weapon::WeaponAffectsMask::ALLIES,
    );
    template.fire_ocl[0] = Some(Arc::new(fire_ocl));
    let mut object = aircraft.owner.write().unwrap();
    let mut set = WeaponTemplateSet::new();
    set.set_weapon_template(WeaponSlotType::Primary, Arc::new(template));
    object.weapon_set.add_weapon_template_set(set);
    object.refresh_weapon_set().unwrap();
    let weapon = object
        .get_weapon_in_slot_mut(WeaponSlotType::Primary)
        .unwrap();
    weapon.set_clip_percent_full(1.0, true);
    weapon.set_status(WeaponStatus::ReadyToFire);
    let source_position = *object.get_position();
    object
        .fire_current_weapon_at_position(&source_position)
        .unwrap();
    assert_eq!(object.get_health(), 75.0);
    assert_eq!(
        *health_at_ocl.lock().unwrap(),
        vec![100.0],
        "CPP FireOCL precedes immediate damage; owned damage still precedes tracker dispatch"
    );
    let weapon = object
        .get_weapon_in_slot_mut(WeaponSlotType::Primary)
        .unwrap();
    assert!(
        weapon.take_pending_self_damage().is_none(),
        "owned self damage is not queued"
    );
    assert!(
        weapon.caller_source(aircraft.id).is_none(),
        "transient source cleared after shot"
    );
}
