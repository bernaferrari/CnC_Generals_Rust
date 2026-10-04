//! AIUpdate.cpp774-780, W3DTankDraw.cpp tread speed-fraction caller.
use super::*;
use crate::object::unit::owned_tread_speed_fixture::{Fixture, child};

#[test]
fn admitted_cached_ai_supplies_body_condition_tread_speed() {
    if !child(concat!(
        module_path!(),
        "::admitted_cached_ai_supplies_body_condition_tread_speed"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = Fixture::new("W3DTankDrawOwnedSpeed", "W3DTankDraw");
    let first = Fixture::actual_draw(&fixture.pristine, "W3DTankDraw");
    let second = Fixture::actual_draw(&fixture.damaged, "W3DTankDraw");
    for handle in [&first, &second] {
        let data = handle.module_data_arc();
        let data = data
            .as_ref()
            .as_any()
            .downcast_ref::<W3DTankDrawModuleData>()
            .expect("Common must invoke the actual registered Draw data proc");
        // INI::parseVelocityReal stores velocity per 30 Hz logic frame.
        assert_eq!(data.tread_animation_rate, 0.25_f32 / 30.0);
    }
    // Invoke the real installed DRAW entry, not the new Object helper directly.
    let draw_speed = |handle: &crate::object::drawable::DrawableModuleHandle, expected_id| {
        handle
            .with_module_downcast::<W3DTankDraw, _, _>(|draw| {
                assert_eq!(
                    draw.owner_id(),
                    Some(expected_id),
                    "actual factory binds this exact Draw to its driving owner"
                );
                draw.do_draw_module(&Matrix3D::IDENTITY);
                assert_eq!(draw.treads.len(), 2, "actual live child tread discovery");
                draw.max_velocity
            })
            .unwrap()
    };
    assert_eq!(
        draw_speed(&first, Fixture::owner_id(&fixture.pristine)),
        40.0,
        "CPP pristine owner uses its current locomotor"
    );
    assert_eq!(
        draw_speed(&second, Fixture::owner_id(&fixture.damaged)),
        23.0,
        "CPP damaged owner uses its damaged speed"
    );
    // The draw must read the SAME cached mutable member, not a copied set.
    let ai = fixture
        .pristine
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    ai.lock()
        .unwrap()
        .with_cur_locomotor_mut(&mut |locomotor| locomotor.set_max_speed(13.0));
    assert_eq!(
        draw_speed(&first, Fixture::owner_id(&fixture.pristine)),
        13.0,
        "CPP runtime max-speed cap is observed immediately"
    );
    assert_eq!(
        draw_speed(&second, Fixture::owner_id(&fixture.damaged)),
        23.0,
        "interleaved real owners do not share the active member"
    );
}
