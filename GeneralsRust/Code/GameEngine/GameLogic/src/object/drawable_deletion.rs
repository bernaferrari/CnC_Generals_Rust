//! Synchronous module cleanup through short Drawable/model borrows.

use super::*;

/// Drawable.cpp:575-586 keeps sibling modules discoverable during delete hooks.
/// The shared-handle path lends only short state borrows; synchronous model
/// resource callbacks run outside both Drawable and installed draw-entry guards.
/// Other module kinds keep their existing on_delete hook and entry borrow.
pub(crate) fn retire_object_drawable(
    drawable: &Arc<RwLock<Drawable>>,
    object_world: Option<Arc<crate::system::engine_stores::WorldServices>>,
) {
    let Ok(state) = drawable.read() else {
        return;
    };
    let id = state.get_drawable_id();
    let world = match state.visual_owner.as_ref() {
        Some(owner) => owner.world_services(),
        None => object_world,
    };
    let modules = state.modules();
    drop(state);
    for entry in modules {
        let particles = entry.with_module(|module| {
            if let Some(model) = module
                .as_any_mut()
                .downcast_mut::<crate::object::draw::W3DModelDraw>()
            {
                Some(model.take_particle_systems_for_deletion())
            } else {
                module.on_delete();
                None
            }
        });
        let Some(particles) = particles else {
            continue;
        };
        let manager = world.as_ref().and_then(|world| {
            world
                .particle_system_manager()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone()
        });
        if let Some(manager) = manager {
            for id in particles {
                manager.destroy_particle_system(id);
            }
        }
        // Preserve on_delete's phase order and visibility. Track take follows
        // particle callbacks; shadow state changes after its release callback.
        let track = with_deleting_model(&entry, |model| model.take_terrain_track_for_deletion());
        if let Some(handle) = track {
            let client = world.as_ref().and_then(|world| {
                world
                    .terrain_track_client()
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone()
            });
            if let Some(client) = client {
                client.unbind_track(handle);
            }
        }
        let decal_client = || {
            world.as_ref().and_then(|world| {
                world
                    .terrain_decal_client()
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone()
            })
        };
        let owner_id = with_deleting_model(&entry, |model| model.owner_id());
        if let Some(id) = owner_id {
            if let Some(client) = decal_client() {
                client.release_unit_shadow(id);
            }
        }
        with_deleting_model(&entry, |model| model.finish_template_shadow_deletion());
        let owner_id = with_deleting_model(&entry, |model| model.owner_id());
        if let Some(id) = owner_id {
            if let Some(client) = decal_client() {
                client.release(id);
            }
        }
    }
    if let Ok(mut drawable) = drawable.write() {
        drawable.modules.clear();
    }
    if let Some(world) = world {
        crate::helpers::TheGameClient::remove_bound_drawable_in(&world, id, drawable);
    }
}

fn with_deleting_model<R>(
    entry: &DrawableModuleHandle,
    operation: impl FnOnce(&mut crate::object::draw::W3DModelDraw) -> R,
) -> R {
    entry.with_module(|module| {
        let model = module
            .as_any_mut()
            .downcast_mut::<crate::object::draw::W3DModelDraw>()
            .expect("prepared deletion changed model kind");
        operation(model)
    })
}
