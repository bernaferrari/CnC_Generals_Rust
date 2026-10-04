//! Actual authored W3D model + cached body particle callback witness. This
//! requires the borrowed W3D binding/reaction prerequisite (hq-5pbog) GREEN;
//! restore only OLD body health dispatch to isolate the intended OLD hang.
use super::*;
use crate::common::types::{EmissionVolumeType, ParticleSystemManagerInterface};
use crate::common::{Matrix3D, Real};
use crate::object::body::body_module::BodyModuleInterface;
use crate::object::draw::w3d_model_draw::{W3DModelDraw, register_pristine_bone_lookup_hook};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Create(u32),
    Position(u32),
    Attach(u32),
    Destroy(u32),
}

struct Manager {
    body: Arc<Mutex<dyn BodyModuleInterface>>,
    drawable: Arc<RwLock<Drawable>>,
    object_id: ObjectID,
    created: AtomicU32,
    attached: Mutex<Vec<(u32, ObjectID)>>,
    events: Mutex<Vec<Event>>,
    expected_rubble: AtomicBool,
    observe_callbacks: AtomicBool,
}
impl std::fmt::Debug for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaxHealthActualParticleWitness")
            .finish_non_exhaustive()
    }
}
impl Manager {
    fn callback(&self) {
        assert!(
            self.body.try_lock().is_ok(),
            "real manager callback must not retain cached body guard"
        );
        let drawable = self
            .drawable
            .try_read()
            .expect("particle callback follows completed Drawable reaction");
        if self.expected_rubble.load(Ordering::SeqCst) {
            assert!(
                drawable
                    .get_model_conditions()
                    .contains(ModelConditionFlags::RUBBLE)
            );
        } else {
            assert!(!drawable.get_model_conditions().intersects(
                ModelConditionFlags::DAMAGED
                    | ModelConditionFlags::REALLYDAMAGED
                    | ModelConditionFlags::RUBBLE
            ));
        }
    }
}
impl ParticleSystemManagerInterface for Manager {
    fn find_template(&self, name: &str) -> Option<u32> {
        match name {
            "MaxHealthOwnerAutoFire" => Some(1),
            "MaxHealthOwnerMediumFire" => Some(2),
            _ => None,
        }
    }
    fn create_particle_system(&self, template: u32) -> Option<u32> {
        self.callback();
        assert_eq!(
            template, 2,
            "CPP aflame small-fire group uses medium-fire template"
        );
        let id = self.created.fetch_add(1, Ordering::SeqCst) + 1;
        self.events.lock().unwrap().push(Event::Create(id));
        Some(id)
    }
    fn create_attached_particle_system_id(&self, _: u32, _: ObjectID) -> Option<u32> {
        panic!("CPP create->position->attach")
    }
    fn find_particle_system(&self, id: u32) -> Option<Box<dyn std::any::Any>> {
        let created = id > 0 && id <= self.created.load(Ordering::SeqCst);
        let destroyed = self.events.lock().unwrap().contains(&Event::Destroy(id));
        if created && !destroyed {
            Some(Box::new(id))
        } else {
            None
        }
    }
    fn set_particle_system_position(&self, id: u32, _: &Coord3D) {
        self.callback();
        assert!((1..=4).contains(&id));
        self.events.lock().unwrap().push(Event::Position(id));
    }
    fn get_particle_system_position(&self, _: u32) -> Option<Coord3D> {
        None
    }
    fn attach_particle_system_to_object(&self, id: u32, owner: ObjectID) {
        self.callback();
        assert_eq!(owner, self.object_id);
        assert_eq!(
            self.body.lock().unwrap().owner_particle_head(),
            if id % 2 == 0 { Some(id - 1) } else { None },
            "CPP attach callback precedes this system's immediate list push"
        );
        self.attached.lock().unwrap().push((id, owner));
        self.events.lock().unwrap().push(Event::Attach(id));
    }
    fn attach_particle_system_to_drawable(&self, _: u32, _: ObjectID) {
        panic!("CPP attaches body particles to Object")
    }
    fn set_particle_system_transform(&self, _: u32, _: &Matrix3D) {
        panic!("CPP sets bone position")
    }
    fn destroy_particle_system(&self, id: u32) {
        if !self.observe_callbacks.load(Ordering::SeqCst) {
            return;
        }
        self.callback();
        assert_eq!(
            self.body.lock().unwrap().owner_particle_head(),
            Some(id),
            "CPP destroy callback precedes removing the current actual head"
        );
        self.events.lock().unwrap().push(Event::Destroy(id));
    }
    fn get_particle_system_emission_volume_type(&self, _: u32) -> Option<EmissionVolumeType> {
        None
    }
    fn set_particle_system_emission_volume_sphere_radius(&self, _: u32, _: Real) {}
    fn set_particle_system_emission_volume_cylinder_radius(&self, _: u32, _: Real) {}
}
// Disable operation-specific callback assertions before independent legacy
// Object retirement; this guard also runs on assertion unwinding.
struct CallbackObservation(Arc<Manager>);
impl Drop for CallbackObservation {
    fn drop(&mut self) {
        self.0.observe_callbacks.store(false, Ordering::SeqCst);
    }
}
struct PristineHook;
impl Drop for PristineHook {
    fn drop(&mut self) {
        register_pristine_bone_lookup_hook(None);
    }
}
struct FireRules {
    medium: String,
    prefix: String,
}
impl Drop for FireRules {
    fn drop(&mut self) {
        let mut rules = global_data::write();
        rules.auto_fire_particle_medium_system = self.medium.clone();
        rules.auto_fire_particle_small_prefix = self.prefix.clone();
    }
}

#[test]
fn authored_aflame_body_uses_actual_bones_and_unlocked_manager_callbacks_after_reaction() {
    if !child(concat!(
        module_path!(),
        "::authored_aflame_body_uses_actual_bones_and_unlocked_manager_callbacks_after_reaction"
    )) {
        return;
    }
    assert!(ensure_thing_factory_exists());
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    // A real backend hook poses the authored public bones. No fake Object,
    // fake body, Unit registry or alternative draw implementation is installed.
    // Fresh bounded child owns this previously-uninstalled fixture hook.
    register_pristine_bone_lookup_hook(Some(Arc::new(|model, _, _, bone| {
        if model.eq_ignore_ascii_case("MaxHealthParticleModel") {
            match bone.to_ascii_lowercase().as_str() {
                "fire01" => Some((1, Matrix3D::from_translation(Coord3D::new(1.0, 0.0, 0.0)))),
                "fire02" => Some((2, Matrix3D::from_translation(Coord3D::new(2.0, 0.0, 0.0)))),
                _ => None,
            }
        } else {
            None
        }
    })));
    let _hook = PristineHook;
    {
        let center = crate::upgrade::center::get_upgrade_center();
        let mut center = center.write().unwrap();
        center.new_upgrade(crate::common::AsciiString::from(TRIGGER));
        center.new_upgrade(crate::common::AsciiString::from(
            "Upgrade_MaxHealthParticleCap",
        ));
    }
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&format!(
        "Object MaxHealthActualParticleOwner\nKindOf = INERT\nBody = ActiveBody ActualBody\nMaxHealth = 100\nInitialHealth = 25\nEnd\nBehavior = MaxHealthUpgrade ActualUpgrade\nTriggeredBy = {TRIGGER}\nAddMaxHealth = 50\nChangeType = FULLY_HEAL\nEnd\nBehavior = MaxHealthUpgrade ActualCapUpgrade\nTriggeredBy = Upgrade_MaxHealthParticleCap\nAddMaxHealth = -150\nChangeType = SAME_CURRENTHEALTH\nEnd\nDraw = W3DModelDraw ActualDraw\nDefaultConditionState\nModel = MaxHealthParticleModel\nEnd\nExtraPublicBone = Fire01 Fire02\nEnd\nEnd\n"
    )),1);
    let mut admission = Admission(ObjectFactory::new());
    let id = admission
        .0
        .create_object(
            "MaxHealthActualParticleOwner",
            Coord3D::default(),
            None,
            ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let owner = admission
        .0
        .get_object(id)
        .unwrap()
        .get_base_object()
        .unwrap();
    assert!(Arc::ptr_eq(
        &owner,
        &OBJECT_REGISTRY.get_object(id).unwrap()
    ));
    let entry = owner
        .read()
        .unwrap()
        .find_module_by_name("MaxHealthUpgrade")
        .unwrap();
    let body = owner.read().unwrap().get_body_module().unwrap();
    let drawable = owner
        .read()
        .unwrap()
        .get_drawable()
        .expect("actual factory-created Drawable");
    let draws = drawable.read().unwrap().modules();
    assert_eq!(draws.len(), 1, "one authored Draw module");
    draws[0].with_module(|module| {
        assert!(
            module.as_any().is::<W3DModelDraw>(),
            "actual canonical model runtime"
        )
    });
    assert_eq!(
        owner
            .read()
            .unwrap()
            .get_multi_logical_bone_position("fire", 16)
            .len(),
        2,
        "actual cached authored bones"
    );
    drawable
        .write()
        .unwrap()
        .set_model_condition_state(ModelConditionFlags::DAMAGED);
    let manager = Arc::new(Manager {
        body,
        drawable,
        object_id: id,
        created: AtomicU32::new(0),
        attached: Mutex::new(Vec::new()),
        events: Mutex::new(Vec::new()),
        expected_rubble: AtomicBool::new(false),
        observe_callbacks: AtomicBool::new(true),
    });
    let _callbacks = CallbackObservation(manager.clone());
    assert!(
        crate::helpers::register_particle_system_manager(manager.clone()),
        "fresh child actual bridge registration"
    );
    let _rules = Rules::install();
    let _fire_rules = {
        let mut rules = global_data::write();
        let old = FireRules {
            medium: rules.auto_fire_particle_medium_system.clone(),
            prefix: rules.auto_fire_particle_small_prefix.clone(),
        };
        rules.auto_fire_particle_medium_system = "MaxHealthOwnerMediumFire".to_owned();
        rules.auto_fire_particle_small_prefix = "fire".to_owned();
        old
    };
    {
        let mut owner = owner.write().unwrap();
        owner.set_status(ObjectStatusMaskType::AFLAME, true);
        owner.set_effectively_dead(true);
        owner.apply_upgrade_modules(mask());
        assert_completed(&owner, &entry);
    }
    assert_eq!(
        manager.created.load(Ordering::SeqCst),
        2,
        "CPP aflame doubles actual small-fire count"
    );
    assert_eq!(*manager.attached.lock().unwrap(), [(1, id), (2, id)]);
    assert_eq!(manager.body.lock().unwrap().owner_particle_head(), Some(2));
    // Invoke the second actual authored upgrade to clip max/current to zero.
    // This goes through the same canonical cap transition, not a synthetic
    // particle-list setup or manually copied body state.
    manager.expected_rubble.store(true, Ordering::SeqCst);
    let cap_mask = UpgradeMaskType::from_bits_retain(
        crate::upgrade::upgrade_mask_for_name("Upgrade_MaxHealthParticleCap").to_bits(),
    );
    {
        let mut owner = owner.write().unwrap();
        owner.apply_upgrade_modules(cap_mask);
        let body = owner.get_body_module().unwrap();
        let body = body.lock().unwrap();
        assert_eq!(
            (
                body.get_health(),
                body.get_max_health(),
                body.get_previous_health()
            ),
            (0.0, 0.0, 150.0)
        );
        assert_eq!(body.get_damage_state(), BodyDamageType::Rubble);
        assert!(owner.is_effectively_dead());
        assert_eq!(body.owner_particle_head(), Some(4));
    }
    assert_eq!(
        *manager.attached.lock().unwrap(),
        [(1, id), (2, id), (3, id), (4, id)]
    );
    assert_eq!(
        *manager.events.lock().unwrap(),
        [
            Event::Create(1),
            Event::Position(1),
            Event::Attach(1),
            Event::Create(2),
            Event::Position(2),
            Event::Attach(2),
            Event::Destroy(2),
            Event::Destroy(1),
            Event::Create(3),
            Event::Position(3),
            Event::Attach(3),
            Event::Create(4),
            Event::Position(4),
            Event::Attach(4),
        ]
    );
}
