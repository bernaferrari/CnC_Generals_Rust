//! Camera effects at the existing FXList.cpp nugget observation point.
//! These are synchronous loans, never a published active-view selection.
use super::*;
use crate::message_stream::game_message::Coord3D as MessageCoord3D;

/// The driving client supplies camera, drawable and visibility observations.
/// Catalog lookups return owned definitions so no catalog guard survives a
/// nugget callback. BoneFX recursively uses these same receiving services.
pub struct FxViewServices<'a> {
    pub add_scorch: &'a mut dyn FnMut([f32; 3], f32, i32),
    pub shake: &'a mut dyn FnMut(&Coord3D, ViewShakeKind),
    pub find_list: &'a mut dyn FnMut(&str) -> Option<Arc<FXList>>,
    pub bone_transforms: &'a mut dyn FnMut(u32, &str, i32, usize) -> Vec<Matrix3D>,
    pub position_visible: &'a mut dyn FnMut(Option<&Coord3D>) -> bool,
    pub create_light_pulse: &'a mut dyn FnMut(DisplayLightPulse) -> bool,
    pub play_fx_audio: &'a mut dyn FnMut(&str, Option<MessageCoord3D>) -> bool,
    /// Receiving client's presentation random stream.
    /// Shared with the view's shake sink, which draws from the same stream.
    pub client_random: &'a std::cell::RefCell<game_engine::common::random_value::RandomState>,
}

/// Retained Core entrypoints still use their own tactical driver. The adapter
/// borrows no View across callbacks: another nugget can synchronously invoke
/// sound, particles or another FXList without a reentrant View borrow.
pub(super) fn with_tactical_fx_view_services<R>(
    run: impl FnOnce(&mut FxViewServices<'_>) -> R,
) -> R {
    // Ownerless Core entrypoint: shake draws use a private stream.
    let mut shake_random = game_engine::common::random_value::RandomState::default();
    let mut shake = |position: &Coord3D, kind| {
        with_tactical_view(|view| {
            view.shake(
                &ViewPoint3::new(position.x, position.y, position.z),
                kind,
                &mut shake_random,
            );
        });
    };
    let mut find_list = |name: &str| crate::fx_list::with_fx_list_store(|s| s.find_fx_list(name));
    // This retained Core entrypoint has no receiving GameClient borrow.
    // Leave bone lookup absent; FXListAtBonePos keeps its GameLogic Drawable fallback.
    let mut bones = |_, _: &str, _, _| Vec::new();
    let mut add_scorch = |position, radius, kind| {
        crate::terrain::scorch_mesh::add_terrain_scorch(position, radius, kind);
    };
    // Ownerless Core entrypoint: no receiving FxClientState is reachable, so
    // light pulses go to a discarded scene and FX audio is not played.
    let client_random = std::cell::RefCell::new(game_engine::common::random_value::RandomState::default());
    let mut ephemeral = DisplayLightScene::default();
    let mut create_light_pulse = |pulse: DisplayLightPulse| ephemeral.create_pulse(pulse);
    let mut play_fx_audio = |_name: &str, _position: Option<MessageCoord3D>| false;
    run(&mut FxViewServices {
        add_scorch: &mut add_scorch,
        shake: &mut shake,
        find_list: &mut find_list,
        bone_transforms: &mut bones,
        position_visible: &mut fx_pos_cell_is_clear,
        create_light_pulse: &mut create_light_pulse,
        play_fx_audio: &mut play_fx_audio,
        client_random: &client_random,
    })
}

impl FXList {
    /// Original positional visibility gate, then immediate authored order.
    /// No whole-list preflight, command extraction or deferred second pass.
    pub fn do_fx_pos_with_view(
        &self,
        primary: Option<&Coord3D>,
        matrix: Option<&Matrix3D>,
        speed: f32,
        secondary: Option<&Coord3D>,
        radius: f32,
        view: &mut FxViewServices<'_>,
    ) {
        if !(view.position_visible)(primary) {
            return;
        }
        self.do_fx_pos_visible_with_view(primary, matrix, speed, secondary, radius, view);
    }

    pub(super) fn do_fx_pos_visible_with_view(
        &self,
        primary: Option<&Coord3D>,
        matrix: Option<&Matrix3D>,
        speed: f32,
        secondary: Option<&Coord3D>,
        radius: f32,
        view: &mut FxViewServices<'_>,
    ) {
        for nugget in &self.nuggets {
            nugget.do_fx_pos_with_view(primary, matrix, speed, secondary, radius, view);
        }
    }

    /// Core-object form retains the existing object visibility policy.
    pub fn do_fx_obj_with_view(
        &self,
        primary: Option<&Object>,
        secondary: Option<&Object>,
        view: &mut FxViewServices<'_>,
    ) {
        if !fx_obj_is_visible(primary) {
            return;
        }
        self.do_fx_obj_visible_with_view(primary, secondary, view);
    }

    pub(super) fn do_fx_obj_visible_with_view(
        &self,
        primary: Option<&Object>,
        secondary: Option<&Object>,
        view: &mut FxViewServices<'_>,
    ) {
        for nugget in &self.nuggets {
            nugget.do_fx_obj_with_view(primary, secondary, view);
        }
    }

    /// Host form consumes the driving world's object observation, without
    /// resolving an ObjectId in a foreign object registry.
    pub fn do_fx_obj_host_with_view(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
        view: &mut FxViewServices<'_>,
    ) {
        if primary.is_shrouded {
            return;
        }
        for nugget in &self.nuggets {
            nugget.do_fx_obj_host_with_view(primary, secondary, view);
        }
    }
}

#[cfg(test)]
#[path = "fx_view_services_tests.rs"]
mod tests;
