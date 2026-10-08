//! Borrowed current-bone queries over installed W3D modules and Drawable skeleton metadata.
//! These do not assert current animated renderer pose.
use super::*;
use crate::object::draw::draw_module::ModelDrawContext;
use crate::object::draw::w3d_model_draw::{
    ModelConditionInfo, PristineBoneInfo, W3DModelDraw, W3DModelDrawModuleData,
};
use crate::object::registry::OBJECT_REGISTRY;
use std::sync::{Arc, RwLock};

const OWNER_ID: ObjectID = 0xA54_9101;
const DRIVING_DRAWABLE_ID: DrawableID = 0xA54_9201;
const DECOY_DRAWABLE_ID: DrawableID = 0xA54_9202;
const BONE: &str = "owner_joint";

struct Registered(ObjectID);
impl Drop for Registered {
    fn drop(&mut self) {
        OBJECT_REGISTRY.unregister_object(self.0);
    }
}

fn add_model_draw(drawable: &mut Drawable, name: &str, has_bone: bool) -> DrawableModuleHandle {
    let mut data = W3DModelDrawModuleData::new();
    data.default_state = 0;
    let mut state = ModelConditionInfo::new();
    state.conditions_yes.push(ModelConditionFlags::empty());
    state.model_name = AsciiString::from("FixtureMetadataModel");
    // This authored-like row only decides whether the W3D interface reports a
    // bone index. The asserted world transform comes from the driving
    // Drawable skeleton below; it is not renderer asset/HTree pose evidence.
    if has_bone {
        state.pristine_bones.insert(
            NameKeyGenerator::name_to_key(BONE),
            PristineBoneInfo {
                transform: Matrix3D::from_translation(Coord3D::new(700.0, 701.0, 702.0)),
                bone_index: 7,
            },
        );
    }
    data.condition_states.push(state);

    let mut model = W3DModelDraw::new(data.clone());
    // Use the public production owner-binding and condition-state APIs; the
    // fixture does not write private owner_id/current-state fields.
    model.bind_owner_id(OWNER_ID);
    let entry = drawable.add_module(
        ModuleInterfaceType::DRAW,
        "W3DModelDraw".into(),
        name.into(),
        Arc::new(data),
        Box::new(model),
    );
    let context = ModelDrawContext {
        instance_scale: 1.0,
        state_particles: true,
    };
    entry.with_object_draw_interface(|draw| {
        draw.replace_model_condition_state_with_context(&ModelConditionFlags::empty(), &context);
    });
    entry
}

fn drawable_with_three_modules(
    has_bone: [bool; 3],
    drawable_id: DrawableID,
    world_bone: Coord3D,
) -> (Arc<RwLock<Drawable>>, [DrawableModuleHandle; 3]) {
    let mut drawable = Drawable::new(
        drawable_id,
        OWNER_ID,
        "FixtureMetadataModel".into(),
        DrawableType::Static,
    );
    drawable.transform = Matrix3D::from_translation(Coord3D::new(5.0, 6.0, 7.0));
    drawable.skeleton.push(BoneData {
        name: BONE.into(),
        parent_index: -1,
        transform: Matrix3D::IDENTITY,
        inverse_bind_pose: Matrix3D::IDENTITY,
    });
    // get_bone_transform returns drawable.transform * bone_transforms[0].
    drawable
        .bone_transforms
        .push(Matrix3D::from_translation(world_bone));
    let handles = [
        add_model_draw(&mut drawable, "Before", has_bone[0]),
        add_model_draw(&mut drawable, "ActiveMiddle", has_bone[1]),
        add_model_draw(&mut drawable, "After", has_bone[2]),
    ];
    (Arc::new(RwLock::new(drawable)), handles)
}

fn register_object_with_drawable(
    drawable: &Arc<RwLock<Drawable>>,
    id: ObjectID,
) -> Arc<RwLock<crate::object::Object>> {
    let mut object = crate::object::Object::new_test(id, 100.0);
    object.set_drawable(Some(Arc::clone(drawable)));
    let object = Arc::new(RwLock::new(object));
    OBJECT_REGISTRY.register_object(id, &object);
    object
}

fn query_while_active_entry_is_held(
    drawable: &Arc<RwLock<Drawable>>,
    active: &DrawableModuleHandle,
) -> Option<Matrix3D> {
    let drawable_guard = drawable.write().expect("driving Drawable write guard");
    active
        .with_object_draw_interface(|current| {
            let owner = DrawableRenderOwner::new(&drawable_guard, active.entry.as_ref());
            owner.get_current_worldspace_client_bone_positions(current, BONE)
        })
        .expect("active entry implements ObjectDrawInterface")
}

#[test]
fn current_bone_owner_query_preserves_before_active_after_search_order() {
    const CHILD: &str = "GENERALS_CURRENT_BONE_OWNER_ORDER_CHILD";
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    let module = module_path!()
        .strip_prefix(prefix)
        .expect("crate module path");
    let name =
        format!("{module}::current_bone_owner_query_preserves_before_active_after_search_order");
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(&name, CHILD)
    {
        return;
    }
    let _serial = crate::test_sync::lock();

    let expected_local = Coord3D::new(12.0, 13.0, 14.0);
    let expected_world = Coord3D::new(17.0, 19.0, 21.0);

    // Preceding miss -> active middle succeeds while its module mutex is held.
    // Holding the following sibling proves first-success return order.
    let (drawable, [_before, active, after]) =
        drawable_with_three_modules([false, true, true], DRIVING_DRAWABLE_ID, expected_local);
    let following_guard = after.entry.clone();
    let answer =
        following_guard.with_module(|_| query_while_active_entry_is_held(&drawable, &active));
    assert_eq!(answer.unwrap().w_axis.truncate(), expected_world);

    // A valid preceding sibling wins before the active entry is visited. Since
    // all interfaces read the same owner bone, the transform value alone cannot
    // identify the winner; holding active and following entries makes touching
    // either after the first success fail the bounded child.
    let (drawable, [_before, active, after]) =
        drawable_with_three_modules([true, true, true], DRIVING_DRAWABLE_ID + 1, expected_local);
    let following_guard = after.entry.clone();
    let answer =
        following_guard.with_module(|_| query_while_active_entry_is_held(&drawable, &active));
    assert_eq!(answer.unwrap().w_axis.truncate(), expected_world);

    // Preceding and active miss -> following sibling is queried after the
    // active entry, and still reads this exact Drawable's world transform.
    let (drawable, [_before, active, _after]) = drawable_with_three_modules(
        [false, false, true],
        DRIVING_DRAWABLE_ID + 2,
        expected_local,
    );
    let answer = query_while_active_entry_is_held(&drawable, &active);
    assert_eq!(answer.unwrap().w_axis.truncate(), expected_world);
}

#[test]
fn current_bone_owner_query_uses_exact_same_id_driving_drawable() {
    const CHILD: &str = "GENERALS_CURRENT_BONE_OWNER_IDENTITY_CHILD";
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    let module = module_path!()
        .strip_prefix(prefix)
        .expect("crate module path");
    let name = format!("{module}::current_bone_owner_query_uses_exact_same_id_driving_drawable");
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(&name, CHILD)
    {
        return;
    }
    let _serial = crate::test_sync::lock();

    let expected = Coord3D::new(12.0, 13.0, 14.0);
    let decoy_bone = Coord3D::new(80.0, 81.0, 82.0);
    let (driver, [_before, active, _after]) =
        drawable_with_three_modules([false, true, false], DRIVING_DRAWABLE_ID, expected);
    let (_decoy, _) =
        drawable_with_three_modules([false, true, false], DECOY_DRAWABLE_ID, decoy_bone);
    let _decoy_object = register_object_with_drawable(&_decoy, OWNER_ID);
    let _registered = Registered(OWNER_ID);

    // This is an intentional same-ObjectID registry decoy. The owner-aware
    // query must read the Drawable passed to this callback, not the decoy
    // reachable through owner_id. Values are current GameLogic skeleton data,
    // not an asserted animated W3D/HTree pose.
    let answer = query_while_active_entry_is_held(&driver, &active);
    assert_eq!(
        answer,
        Some(driver.read().unwrap().get_bone_transform(BONE).unwrap())
    );
    assert_ne!(
        answer.unwrap().w_axis.truncate(),
        Coord3D::new(85.0, 87.0, 89.0)
    );
}
