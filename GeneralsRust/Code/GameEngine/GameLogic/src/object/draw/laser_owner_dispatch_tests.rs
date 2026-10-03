//! Actual Drawable::draw regressions for W3DLaserDraw.cpp:228-438.
//! Fixtures install the canonical modules through Drawable::add_module. They do
//! not invoke a parallel renderer or publish an active test world.

use super::*;
use crate::drawable::Drawable as DrawableTrait;
use crate::object::drawable::{Drawable, DrawableModuleHandle, DrawableType};
use crate::object::update::laser_update::{LaserUpdateModule, LaserUpdateModuleData};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module::ModuleInterfaceType;
use std::io::Cursor;
use std::sync::{Arc, RwLock};

fn add_update(
    drawable: &mut Drawable,
    mask: ModuleInterfaceType,
    name: &str,
    start: Coord3D,
    end: Coord3D,
    dirty: bool,
) -> DrawableModuleHandle {
    let data = Arc::new(LaserUpdateModuleData::default());
    let mut update =
        LaserUpdateModule::new(NameKeyGenerator::name_to_key(name), data.clone(), None);
    update.update_mut().set_loaded_segment(start, end, dirty);
    drawable.add_module(mask, name.into(), name.into(), data, Box::new(update))
}

fn add_draw(drawable: &mut Drawable, tag: &str) -> DrawableModuleHandle {
    let mut data = W3DLaserDrawModuleData::new();
    data.module_tag_name_key = NameKeyGenerator::name_to_key(tag);
    data.inner_beam_width = 4.0;
    let module = W3DLaserDraw::new(data.clone());
    drawable.add_module(
        ModuleInterfaceType::DRAW,
        "W3DLaserDraw".into(),
        tag.into(),
        Arc::new(data),
        Box::new(module),
    )
}

fn segments(draw: &DrawableModuleHandle) -> Vec<(Coord3D, Coord3D, Real)> {
    draw.with_module_downcast::<W3DLaserDraw, _, _>(|draw| {
        draw.lines
            .iter()
            .map(|line| (line.start, line.end, line.width))
            .collect()
    })
    .expect("canonical W3DLaserDraw")
}

fn is_dirty(update: &DrawableModuleHandle) -> bool {
    update
        .with_module_downcast::<LaserUpdateModule, _, _>(|module| module.update_mut().is_dirty())
        .expect("canonical LaserUpdate")
}

fn set_segment(update: &DrawableModuleHandle, start: Coord3D, end: Coord3D, dirty: bool) {
    update
        .with_module_downcast::<LaserUpdateModule, _, _>(|module| {
            module.update_mut().set_loaded_segment(start, end, dirty);
        })
        .expect("canonical LaserUpdate");
}

#[test]
fn driving_drawable_uses_own_laser_update_with_same_ids_and_held_owner() {
    let _serial = crate::test_sync::lock();
    let start_a = Coord3D::new(10.0, 20.0, 30.0);
    let end_a = Coord3D::new(40.0, 50.0, 60.0);
    let start_b = Coord3D::new(-10.0, -20.0, -30.0);
    let end_b = Coord3D::new(-40.0, -50.0, -60.0);
    let mut a = Drawable::new(93_701, INVALID_ID, "LaserA".into(), DrawableType::Static);
    let mut b = Drawable::new(93_701, INVALID_ID, "LaserB".into(), DrawableType::Static);
    let update_a = add_update(
        &mut a,
        ModuleInterfaceType::CLIENT_UPDATE,
        "LaserUpdate",
        start_a,
        end_a,
        true,
    );
    let update_b = add_update(
        &mut b,
        ModuleInterfaceType::CLIENT_UPDATE,
        "LaserUpdate",
        start_b,
        end_b,
        true,
    );
    let draw_a = add_draw(&mut a, "LaserA");
    let draw_b = add_draw(&mut b, "LaserB");
    let a = Arc::new(RwLock::new(a));
    let b = Arc::new(RwLock::new(b));
    assert!(is_dirty(&update_a));
    assert!(is_dirty(&update_b));
    assert!(segments(&draw_a).is_empty());
    assert!(segments(&draw_b).is_empty());

    // The production caller already holds this Drawable's write borrow. No
    // owner lookup or relock may occur inside the canonical laser draw module.
    a.write().unwrap().draw(None);
    assert_eq!(segments(&draw_a), vec![(start_a, end_a, 4.0)]);
    assert!(!is_dirty(&update_a));
    assert!(is_dirty(&update_b));
    b.write().unwrap().draw(None);
    assert_eq!(segments(&draw_b), vec![(start_b, end_b, 4.0)]);
    assert!(!is_dirty(&update_b));

    set_segment(&update_a, end_a, start_a, true);
    a.write().unwrap().draw(None);
    assert_eq!(segments(&draw_a), vec![(end_a, start_a, 4.0)]);
    assert_eq!(segments(&draw_b), vec![(start_b, end_b, 4.0)]);
    a.write().unwrap().clear_modules();
    b.write().unwrap().draw(None);
    assert_eq!(segments(&draw_b), vec![(start_b, end_b, 4.0)]);
    b.write().unwrap().clear_modules();
}

#[test]
fn laser_dirty_consumption_follows_draw_ordinal_and_self_dirty() {
    let _serial = crate::test_sync::lock();
    let start = Coord3D::new(1.0, 2.0, 3.0);
    let end = Coord3D::new(4.0, 5.0, 6.0);
    let changed = Coord3D::new(40.0, 50.0, 60.0);
    let mut owner = Drawable::new(93_702, INVALID_ID, "Laser".into(), DrawableType::Static);
    let update = add_update(
        &mut owner,
        ModuleInterfaceType::CLIENT_UPDATE,
        "LaserUpdate",
        start,
        end,
        true,
    );
    let first = add_draw(&mut owner, "FirstLaser");
    let second = add_draw(&mut owner, "SecondLaser");
    owner.draw(None);
    assert_eq!(segments(&first), vec![(start, end, 4.0)]);
    assert_eq!(segments(&second), vec![(start, end, 4.0)]);

    // CPP clears update dirty at the first DRAW. The second has no selfDirty,
    // so it retains its existing line rather than consuming a frame snapshot.
    set_segment(&update, start, changed, true);
    owner.draw(None);
    assert!(!is_dirty(&update));
    assert_eq!(segments(&first), vec![(start, changed, 4.0)]);
    assert_eq!(segments(&second), vec![(start, end, 4.0)]);
    second
        .with_module_downcast::<W3DLaserDraw, _, _>(|draw| {
            Snapshotable::load_post_process(draw).unwrap();
        })
        .unwrap();
    owner.draw(None);
    assert_eq!(segments(&second), vec![(start, changed, 4.0)]);

    // A clean sibling does not reset points on repeated draws.
    set_segment(&update, end, start, false);
    owner.draw(None);
    assert_eq!(segments(&first), vec![(start, changed, 4.0)]);
    assert_eq!(segments(&second), vec![(start, changed, 4.0)]);
    owner.clear_modules();
}

#[test]
fn laser_lookup_uses_named_client_update_bucket() {
    let _serial = crate::test_sync::lock();
    let start = Coord3D::new(1.0, 2.0, 3.0);
    let end = Coord3D::new(4.0, 5.0, 6.0);
    let decoy = Coord3D::new(1000.0, 2000.0, 3000.0);
    let mut owner = Drawable::new(93_703, INVALID_ID, "Laser".into(), DrawableType::Static);
    let wrong_bucket = add_update(
        &mut owner,
        ModuleInterfaceType::DRAW,
        "LaserUpdate",
        decoy,
        decoy,
        true,
    );
    let wrong_name = add_update(
        &mut owner,
        ModuleInterfaceType::CLIENT_UPDATE,
        "OtherUpdate",
        decoy,
        decoy,
        true,
    );
    // Descriptor names are diagnostic metadata. CPP lookup compares the
    // canonical module's getModuleNameKey rather than the descriptor label.
    let data = Arc::new(LaserUpdateModuleData::default());
    let mut module = LaserUpdateModule::new(
        NameKeyGenerator::name_to_key("LaserUpdate"),
        data.clone(),
        None,
    );
    module.update_mut().set_loaded_segment(start, end, true);
    let expected = owner.add_module(
        ModuleInterfaceType::CLIENT_UPDATE,
        "DifferentDescriptorName".into(),
        "ActualLaserUpdate".into(),
        data,
        Box::new(module),
    );
    let draw = add_draw(&mut owner, "Laser");
    owner.draw(None);
    assert_eq!(segments(&draw), vec![(start, end, 4.0)]);
    assert!(!is_dirty(&expected));
    assert!(is_dirty(&wrong_name));
    assert!(is_dirty(&wrong_bucket));
    owner.clear_modules();
}

#[test]
fn first_named_laser_update_stops_lookup_even_when_clean() {
    let _serial = crate::test_sync::lock();
    let start = Coord3D::new(1.0, 2.0, 3.0);
    let end = Coord3D::new(4.0, 5.0, 6.0);
    let later_end = Coord3D::new(400.0, 500.0, 600.0);
    let mut owner = Drawable::new(93_707, INVALID_ID, "Laser".into(), DrawableType::Static);
    let first = add_update(
        &mut owner,
        ModuleInterfaceType::CLIENT_UPDATE,
        "LaserUpdate",
        start,
        end,
        false,
    );
    let later = add_update(
        &mut owner,
        ModuleInterfaceType::CLIENT_UPDATE,
        "LaserUpdate",
        start,
        later_end,
        true,
    );
    let draw = add_draw(&mut owner, "Laser");
    owner.draw(None);
    assert_eq!(segments(&draw), vec![(start, end, 4.0)]);
    assert!(!is_dirty(&first));
    assert!(is_dirty(&later));
    // With selfDirty cleared, the first match declines a reset. It still wins
    // discovery; a later dirty module with the same key must not be consumed.
    owner.draw(None);
    assert_eq!(segments(&draw), vec![(start, end, 4.0)]);
    assert!(is_dirty(&later));
    owner.clear_modules();
}

#[test]
fn missing_named_laser_update_returns_without_consuming_other_interfaces() {
    let _serial = crate::test_sync::lock();
    let start = Coord3D::new(1.0, 2.0, 3.0);
    let mut owner = Drawable::new(93_704, INVALID_ID, "Laser".into(), DrawableType::Static);
    let unrelated = add_update(
        &mut owner,
        ModuleInterfaceType::CLIENT_UPDATE,
        "OtherUpdate",
        start,
        start,
        true,
    );
    let draw = add_draw(&mut owner, "Laser");
    owner.draw(None);
    assert!(segments(&draw).is_empty());
    assert!(is_dirty(&unrelated));
    owner.clear_modules();
}

#[test]
fn laser_width_reset_consumes_current_canonical_update_width() {
    let _serial = crate::test_sync::lock();
    let start = Coord3D::new(1.0, 2.0, 3.0);
    let end = Coord3D::new(4.0, 5.0, 6.0);
    let mut owner = Drawable::new(93_705, INVALID_ID, "Laser".into(), DrawableType::Static);
    let update = add_update(
        &mut owner,
        ModuleInterfaceType::CLIENT_UPDATE,
        "LaserUpdate",
        start,
        end,
        true,
    );
    let draw = add_draw(&mut owner, "Laser");
    update
        .with_module_downcast::<LaserUpdateModule, _, _>(|module| {
            module
                .update_mut()
                .init_laser(None, None, Some(&start), Some(&end), String::new(), 10);
        })
        .unwrap();
    owner.draw(None);
    assert_eq!(segments(&draw), vec![(start, end, 0.0)]);
    assert!(!is_dirty(&update));
    // Negative duration starts decay at full width and marks the real update
    // dirty; the next draw must read that width, even with unchanged endpoints.
    update
        .with_module_downcast::<LaserUpdateModule, _, _>(|module| {
            module.update_mut().init_laser(
                None,
                None,
                Some(&start),
                Some(&end),
                String::new(),
                -10,
            );
        })
        .unwrap();
    owner.draw(None);
    assert_eq!(segments(&draw), vec![(start, end, 4.0)]);
    assert!(!is_dirty(&update));
    owner.draw(None);
    assert_eq!(segments(&draw), vec![(start, end, 4.0)]);
    owner.clear_modules();
}

#[test]
fn laser_xfer_keeps_runtime_input_out_of_draw_payload_and_resets_after_load() {
    let _serial = crate::test_sync::lock();
    let start = Coord3D::new(1.0, 2.0, 3.0);
    let end = Coord3D::new(4.0, 5.0, 6.0);
    let mut owner = Drawable::new(93_706, INVALID_ID, "Laser".into(), DrawableType::Static);
    let update = add_update(
        &mut owner,
        ModuleInterfaceType::CLIENT_UPDATE,
        "LaserUpdate",
        start,
        end,
        true,
    );
    let draw = add_draw(&mut owner, "Laser");
    owner.draw(None);
    let mut bytes = Vec::new();
    draw.with_module(|module| {
        let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
        save.open("laser_draw").unwrap();
        module.xfer(&mut save).unwrap();
        save.close().unwrap();
    });
    assert_eq!(
        bytes,
        vec![1, 1, 1, 1],
        "CPP draw and three base version bytes only"
    );
    draw.with_module(|module| {
        let mut load = XferLoad::new(Cursor::new(bytes), 1);
        load.open("laser_draw").unwrap();
        module.xfer(&mut load).unwrap();
        load.close().unwrap();
        module.load_post_process().unwrap();
    });
    let changed = Coord3D::new(40.0, 50.0, 60.0);
    set_segment(&update, start, changed, false);
    owner.draw(None);
    assert_eq!(segments(&draw), vec![(start, changed, 4.0)]);
    assert!(!is_dirty(&update));
    owner.clear_modules();
}
