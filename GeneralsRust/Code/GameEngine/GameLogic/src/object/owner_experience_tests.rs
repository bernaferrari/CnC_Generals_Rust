//! C++ ExperienceTracker.cpp: explicit levels use owner thresholds; forwarding
//! scales at each hop and promotes the sink with its own template and effects.
use super::*;
use crate::common::DefaultThingTemplate;
use crate::object::registry::OBJECT_REGISTRY;

fn tracked(id: ObjectID, trainable: bool, required: &str) -> Object {
    let mut object = Object::new_test(id, 100.0);
    let mut template = DefaultThingTemplate::new(format!("OwnerExperience{id}"));
    template.parse_object_fields_from_ini(&std::collections::HashMap::from([
        (
            "IsTrainable".to_owned(),
            if trainable { "Yes" } else { "No" }.to_owned(),
        ),
        ("ExperienceRequired".to_owned(), required.to_owned()),
    ]));
    object.set_template_for_test(Arc::new(template));
    object.experience_tracker = Some(Box::new(ExperienceTracker::new(id)));
    object
}

struct Registered(ObjectID);
impl Drop for Registered {
    fn drop(&mut self) {
        OBJECT_REGISTRY.unregister_object(self.0);
    }
}

fn register(object: Object) -> (Arc<RwLock<Object>>, Registered) {
    let id = object.get_id();
    let object = Arc::new(RwLock::new(object));
    OBJECT_REGISTRY.register_object(id, &object);
    (object, Registered(id))
}

fn xp(object: &Object) -> i32 {
    object
        .with_experience_tracker(|tracker| tracker.get_current_experience())
        .unwrap()
}

#[test]
fn explicit_level_uses_owned_template_instead_of_degraded_defaults() {
    let mut object = tracked(88901, false, "0 11 23 47");
    assert!(object.set_veterancy_level_with_side_effects(VeterancyLevel::Elite, false));
    assert_eq!(xp(&object), 23);
    assert!(object.test_weapon_set_flag(WeaponSetType::Elite));
}

#[test]
fn sink_uses_its_own_trainability_thresholds_and_promotion_effects() {
    let mut sink = tracked(88902, true, "0 40 80 160");
    sink.with_experience_tracker_mut(|tracker| tracker.set_experience_scalar(2.0));
    let (sink, _registered) = register(sink);
    let mut source = tracked(88903, false, "0 999 1999 2999");
    source.with_experience_tracker_mut(|tracker| {
        tracker.set_experience_sink(88902);
        tracker.set_experience_scalar(2.0);
    });
    // Source does not promote; C++ unconditionally scales 10 -> 20 while
    // forwarding, then the trainable sink's bonus scales 20 -> 40.
    assert!(!source.add_experience_points_with_side_effects(10, true));
    assert_eq!(xp(&source), 0);
    let sink = sink.read().unwrap();
    assert_eq!(xp(&sink), 40);
    assert_eq!(sink.get_veterancy_level(), VeterancyLevel::Veteran);
    assert!(sink.test_weapon_set_flag(WeaponSetType::Veteran));
    assert!(!source.test_weapon_set_flag(WeaponSetType::Veteran));
}

#[test]
fn tracker_adapter_forwards_without_reading_the_write_locked_sink() {
    let (sink, _registered) = register(tracked(88904, true, "0 7 14 28"));
    let mut source = ExperienceTracker::new(88905);
    source.set_experience_sink(88904);
    assert_eq!(
        source.add_experience_points(7, false, &[0, 999, 1999, 2999]),
        None
    );
    let sink = sink.read().unwrap();
    assert_eq!(xp(&sink), 7);
    assert_eq!(sink.get_veterancy_level(), VeterancyLevel::Veteran);
    assert!(sink.test_weapon_set_flag(WeaponSetType::Veteran));
}

#[test]
fn missing_sink_does_not_make_an_untrainable_source_trainable() {
    let mut source = tracked(88906, false, "0 5 10 20");
    source.with_experience_tracker_mut(|tracker| tracker.set_experience_sink(88907));
    assert!(!source.add_experience_points_with_side_effects(5, true));
    assert_eq!(xp(&source), 0);
}

#[test]
fn forwarding_without_bonus_still_applies_the_source_scalar() {
    let mut sink = tracked(88908, true, "0 20 40 80");
    sink.with_experience_tracker_mut(|tracker| tracker.set_experience_scalar(10.0));
    let (sink, _registered) = register(sink);
    let mut source = tracked(88909, false, "0 999 1999 2999");
    source.with_experience_tracker_mut(|tracker| {
        tracker.set_experience_sink(88908);
        tracker.set_experience_scalar(2.0);
    });
    source.add_experience_points_with_side_effects(10, false);
    assert_eq!(xp(&sink.read().unwrap()), 20);
}

#[test]
fn missing_sink_falls_back_to_the_trainable_sources_own_template() {
    let mut source = tracked(88910, true, "0 5 10 20");
    source.with_experience_tracker_mut(|tracker| tracker.set_experience_sink(88911));
    assert!(source.add_experience_points_with_side_effects(5, false));
    assert_eq!(xp(&source), 5);
    assert!(source.test_weapon_set_flag(WeaponSetType::Veteran));
}

#[test]
fn equal_ids_in_separate_owned_objects_keep_their_own_thresholds() {
    let mut first = tracked(88912, false, "0 11 23 47");
    let mut second = tracked(88912, false, "0 40 80 160");
    first.set_veterancy_level_with_side_effects(VeterancyLevel::Elite, false);
    second.set_veterancy_level_with_side_effects(VeterancyLevel::Veteran, false);
    assert_eq!(xp(&first), 23);
    assert_eq!(xp(&second), 40);
    first.set_veterancy_level_with_side_effects(VeterancyLevel::Regular, false);
    assert_eq!(xp(&second), 40);
}

#[test]
fn level_gain_uses_the_already_write_locked_owner_and_clamps_to_heroic() {
    let (object, _registered) = register(tracked(88913, true, "0 7 21 35"));
    let mut object = object.write().unwrap();
    assert!(object.gain_exp_for_level_with_side_effects(1, false));
    assert_eq!(xp(&object), 7);
    assert!(object.test_weapon_set_flag(WeaponSetType::Veteran));
    assert!(object.gain_exp_for_level_with_side_effects(100, false));
    assert_eq!(xp(&object), 35);
    assert_eq!(object.get_veterancy_level(), VeterancyLevel::Heroic);
    assert!(object.test_weapon_set_flag(WeaponSetType::Hero));
    assert!(!object.gain_exp_for_level_with_side_effects(1, false));
}

#[test]
fn level_gain_returns_cpp_request_result_even_when_untrainable() {
    let mut object = tracked(88914, false, "0 7 21 35");
    assert!(object.gain_exp_for_level_with_side_effects(1, false));
    assert_eq!(xp(&object), 0);
    assert!(!object.gain_exp_for_level_with_side_effects(0, false));
    assert!(!object.gain_exp_for_level_with_side_effects(-1, false));
}

#[test]
fn chained_sinks_scale_at_each_hop_and_only_promote_the_final_owner() {
    let mut sink = tracked(88915, true, "0 30 60 120");
    sink.with_experience_tracker_mut(|tracker| tracker.set_experience_scalar(2.0));
    let (sink, _registered_sink) = register(sink);
    let mut middle = tracked(88916, false, "0 999 1999 2999");
    middle.with_experience_tracker_mut(|tracker| {
        tracker.set_experience_sink(88915);
        tracker.set_experience_scalar(1.5);
    });
    let (middle, _registered_middle) = register(middle);
    let mut source = tracked(88917, false, "0 999 1999 2999");
    source.with_experience_tracker_mut(|tracker| {
        tracker.set_experience_sink(88916);
        tracker.set_experience_scalar(2.0);
    });
    source.add_experience_points_with_side_effects(5, true);
    assert_eq!(xp(&source), 0);
    assert_eq!(xp(&middle.read().unwrap()), 0);
    let sink = sink.read().unwrap();
    assert_eq!(xp(&sink), 30);
    assert!(sink.test_weapon_set_flag(WeaponSetType::Veteran));
}

#[test]
fn tracker_reset_adapter_applies_effects_to_the_sink_not_the_source() {
    let (sink, _registered) = register(tracked(88918, true, "0 7 14 28"));
    let mut source = ExperienceTracker::new(88919);
    source.set_experience_sink(88918);
    assert_eq!(
        source.set_experience_and_level(14, &[0, 999, 1999, 2999]),
        None
    );
    assert_eq!(source.get_current_experience(), 0);
    let sink = sink.read().unwrap();
    assert_eq!(xp(&sink), 14);
    assert!(sink.test_weapon_set_flag(WeaponSetType::Elite));
}
