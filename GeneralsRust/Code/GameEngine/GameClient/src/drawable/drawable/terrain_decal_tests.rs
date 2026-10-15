//! Drawable.cpp decal callbacks, first Draw selection, numeric rules and Xfer phase.
use super::*;
use crate::core::game_client::GameClient;
use crate::effects::particle_manager::ParticleSystemManager;
use game_engine::common::bit_flags::ModelConditionFlags;
use game_engine::common::system::{Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::{
    ThingTemplate,
    module::{BaseModuleData, ModuleInterfaceType},
};
use gamelogic::object::update::AnimatedParticleSysBoneClientUpdateModule;
use std::io::{self, Cursor, Read};
use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, Ordering},
};

#[derive(Debug, Default)]
struct Events {
    sequence: AtomicU64,
    opacity: AtomicU32,
    read_position: AtomicU64,
    decal_position: AtomicU64,
}
impl Events {
    fn push(&self, event: u64) {
        self.sequence
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                assert_eq!(old >> 60, 0, "bounded callback sequence overflow");
                Some((old << 4) | event)
            })
            .unwrap();
    }
}
#[derive(Debug)]
struct DrawProbe {
    ordinal: u64,
    events: Arc<Events>,
}
impl DrawModule for DrawProbe {
    fn replace_model_condition_state(&mut self, _: &ModelConditionBitFlags) {
        self.events.push(self.ordinal);
    }
    fn set_terrain_decal(&mut self, kind: TerrainDecalType) {
        self.events.push(if kind == TerrainDecalType::None {
            12 + self.ordinal
        } else {
            2 + self.ordinal
        });
        self.events.decal_position.store(
            self.events.read_position.load(Ordering::SeqCst),
            Ordering::SeqCst,
        );
    }
    fn set_terrain_decal_size(&mut self, _: f32, _: f32) {
        self.events.push(5 + self.ordinal);
    }
    fn set_terrain_decal_opacity(&mut self, opacity: f32) {
        self.events.push(8 + self.ordinal);
        self.events
            .opacity
            .store(opacity.to_bits(), Ordering::SeqCst);
    }
}
fn receiver(cu: bool) -> (GameClient, DrawableId) {
    let mut template = ThingTemplate::new();
    if cu {
        template.add_client_update_module_info(
            "AnimatedParticleSysBoneClientUpdate".into(),
            "ActualCU".into(),
            Arc::new(BaseModuleData::new()),
            ModuleInterfaceType::CLIENT_UPDATE,
        );
    }
    let mut client = GameClient::new().unwrap();
    let id = client.create_drawable_from_template(&template).unwrap();
    (client, id)
}
fn basic(client: &mut GameClient, id: DrawableId) -> &mut BasicDrawable {
    client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}
fn probes(drawable: &mut BasicDrawable, count: u64) -> Arc<Events> {
    let events = Arc::new(Events::default());
    for ordinal in 1..=count {
        drawable.add_draw_module(Box::new(DrawProbe {
            ordinal,
            events: events.clone(),
        }));
    }
    events
}
fn step(drawable: &mut BasicDrawable, particles: &mut ParticleSystemManager, frame: u32) {
    drawable.set_current_frame(frame);
    drawable
        .update_native_client_modules(Some(particles), false, None)
        .unwrap();
    drawable.update_after_client_modules(frame);
}
fn payload(drawable: &mut BasicDrawable) -> Vec<u8> {
    let mut bytes = Vec::new();
    drawable
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 7))
        .unwrap();
    bytes
}
fn fade(drawable: &BasicDrawable) -> (TerrainDecalType, f32, f32, f32) {
    (
        drawable.terrain_decal_type,
        drawable.decal_opacity,
        drawable.decal_opacity_fade_rate,
        drawable.decal_opacity_fade_target,
    )
}

#[test]
fn receiving_decal_setter_flushes_before_first_draw_and_equal_type_is_inert() {
    let (mut client, id) = receiver(true);
    let (mut peer, peer_id) = receiver(true);
    assert_eq!(id, peer_id);
    let events = probes(basic(&mut client, id), 2);
    let peer_events = probes(basic(&mut peer, peer_id), 2);
    basic(&mut client, id).set_terrain_decal(TerrainDecalType::ShadowTexture);
    assert_eq!(
        events.sequence.load(Ordering::SeqCst),
        0x123,
        "conditions precede first actual Draw decal callback; CU and second Draw get no decal"
    );
    basic(&mut client, id).set_model_condition_state(ModelConditionFlags::MOVING);
    basic(&mut client, id).set_terrain_decal(TerrainDecalType::ShadowTexture);
    assert_eq!(
        events.sequence.load(Ordering::SeqCst),
        0x123,
        "equal type is inert even while dirty"
    );
    basic(&mut client, id).get_draw_modules_mut();
    assert_eq!(events.sequence.load(Ordering::SeqCst), 0x12312);
    assert_eq!(peer_events.sequence.load(Ordering::SeqCst), 0);
    assert_eq!(
        basic(&mut peer, peer_id).terrain_decal_type,
        TerrainDecalType::None
    );
}

#[test]
fn receiving_decal_size_flushes_and_routes_to_first_actual_draw() {
    let (mut client, id) = receiver(true);
    let events = probes(basic(&mut client, id), 2);
    basic(&mut client, id).set_terrain_decal_size(4., 7.);
    assert_eq!(
        events.sequence.load(Ordering::SeqCst),
        0x126,
        "size must flush conditions then skip the installed CU and use first Draw only"
    );
    assert_eq!(
        basic(&mut client, id).terrain_decal_size,
        Vector3::new(4., 7., 0.)
    );
    basic(&mut client, id).set_terrain_decal_size(4., 7.);
    assert_eq!(
        events.sequence.load(Ordering::SeqCst),
        0x1266,
        "same size still dispatches"
    );
}

#[test]
fn receiving_no_draw_decal_fade_is_frozen_including_actual_cu_only() {
    for cu in [false, true] {
        for rate in [-0.2, 0.4] {
            let (mut client, id) = receiver(cu);
            let (mut restored, restored_id) = receiver(cu);
            let (mut peer, peer_id) = receiver(cu);
            assert_eq!((id, id), (restored_id, peer_id));
            let first = basic(&mut client, id);
            assert_eq!(first.draw_modules.len(), usize::from(cu));
            first.set_terrain_decal(TerrainDecalType::ShadowTexture);
            first.decal_opacity = 0.35;
            first.set_terrain_decal_fade_target(0.7, rate);
            let initial = fade(first);
            let saved = payload(first);
            basic(&mut restored, id)
                .xfer(&mut XferLoad::new(Cursor::new(&saved), 7))
                .unwrap();
            assert_eq!(payload(basic(&mut restored, id)), saved);
            let mut particles = ParticleSystemManager::new();
            for frame in 1..=3 {
                step(basic(&mut client, id), &mut particles, frame);
                step(basic(&mut restored, id), &mut particles, frame);
                assert_eq!(
                    fade(basic(&mut client, id)),
                    initial,
                    "no original Draw module means no decal integration or threshold transition"
                );
                assert_eq!(fade(basic(&mut restored, id)), initial);
                assert_eq!(
                    payload(basic(&mut restored, id)),
                    payload(basic(&mut client, id))
                );
            }
            if cu {
                let module = basic(&mut client, id).draw_modules[0]
                    .logic_module_mut()
                    .unwrap();
                assert_eq!(
                    module
                        .as_any_mut()
                        .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>()
                        .unwrap()
                        .life(),
                    3
                );
            }
            let events = probes(basic(&mut client, id), 1);
            step(basic(&mut client, id), &mut particles, 4);
            assert_eq!(
                f32::from_bits(events.opacity.load(Ordering::SeqCst)),
                initial.1,
                "later admitted Draw starts from preserved old opacity"
            );
            assert_eq!(basic(&mut client, id).decal_opacity, initial.1 + rate);
            assert_eq!(
                fade(basic(&mut peer, peer_id)),
                (TerrainDecalType::None, 0., 0., 0.)
            );
            basic(&mut restored, id).terrain_decal_type = TerrainDecalType::None;
            step(basic(&mut restored, id), &mut particles, 4);
            assert_eq!(
                fade(basic(&mut restored, id)),
                (TerrainDecalType::None, 0., rate, 0.7)
            );
        }
    }
}

#[test]
fn receiving_first_draw_decal_fade_preserves_callbacks_crossings_and_expiry_phase() {
    let (mut client, id) = receiver(true);
    let first = basic(&mut client, id);
    first.set_terrain_decal(TerrainDecalType::ShadowTexture);
    first.decal_opacity = 0.8;
    first.set_terrain_decal_fade_target(0.9, 0.3);
    first.set_expiration_frame(15);
    first.color_flash(Vector3::new(1., 0., 0.), 2);
    let events = probes(first, 2);
    let mut particles = ParticleSystemManager::new();
    step(basic(&mut client, id), &mut particles, 15);
    assert_eq!(
        events.sequence.load(Ordering::SeqCst),
        0x1299,
        "decal update flushes then sends old opacity and final one to first Draw only"
    );
    assert_eq!(
        fade(basic(&mut client, id)),
        (TerrainDecalType::ShadowTexture, 1., 0., 0.9)
    );
    assert_eq!(
        basic(&mut client, id).flash_count,
        2,
        "decal precedes expiry, flash follows it"
    );
    assert!(basic(&mut client, id).is_expired(15));

    let (mut client, id) = receiver(true);
    let first = basic(&mut client, id);
    first.set_terrain_decal(TerrainDecalType::ShadowTexture);
    first.decal_opacity = 0.1;
    first.set_terrain_decal_fade_target(0.9, -0.2);
    let events = probes(first, 2);
    step(basic(&mut client, id), &mut particles, 1);
    assert_eq!(
        events.sequence.load(Ordering::SeqCst),
        0x129d,
        "negative crossing sends old opacity then clears first Draw without a duplicate condition"
    );
    assert_eq!(
        fade(basic(&mut client, id)),
        (TerrainDecalType::None, 0., 0., 0.9)
    );

    let (mut client, id) = receiver(true);
    basic(&mut client, id).set_terrain_decal(TerrainDecalType::ShadowTexture);
    let events = probes(basic(&mut client, id), 2);
    step(basic(&mut client, id), &mut particles, 1);
    assert_eq!(
        events.sequence.load(Ordering::SeqCst),
        0x12,
        "zero rate still requests dirty Draw modules"
    );
    basic(&mut client, id).set_terrain_decal_fade_target(0.3, 0.2);
    for frame in 2..=4 {
        step(basic(&mut client, id), &mut particles, frame);
    }
    assert_eq!(
        basic(&mut client, id).decal_opacity,
        0.6,
        "target is not an integration clamp"
    );
}

struct ObservedReader {
    cursor: Cursor<Vec<u8>>,
    events: Arc<Events>,
}
impl Read for ObservedReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self.cursor.read(output)?;
        self.events
            .read_position
            .store(self.cursor.position(), Ordering::SeqCst);
        Ok(count)
    }
}
#[test]
fn native_decal_load_calls_setter_at_enum_position_then_continues() {
    let (mut source, id) = receiver(true);
    let (mut restored, restored_id) = receiver(true);
    let (mut peer, peer_id) = receiver(true);
    assert_eq!((id, id), (restored_id, peer_id));
    let source_events = probes(basic(&mut source, id), 1);
    let restored_events = probes(basic(&mut restored, id), 1);
    let peer_events = probes(basic(&mut peer, id), 1);
    let first = basic(&mut source, id);
    first.set_terrain_decal(TerrainDecalType::ShadowTexture);
    first.explicit_opacity = f32::from_bits(0x3ed4_87ab);
    first.decal_opacity = 0.8;
    first.set_terrain_decal_fade_target(0.9, 0.3);
    let before_save = source_events.sequence.load(Ordering::SeqCst);
    let saved = payload(first);
    assert_eq!(
        source_events.sequence.load(Ordering::SeqCst),
        before_save,
        "save cannot dispatch decal callbacks"
    );
    let explicit: Vec<_> = saved
        .windows(4)
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == 0x3ed4_87abu32.to_ne_bytes()).then_some(index))
        .collect();
    assert_eq!(explicit.len(), 1);
    basic(&mut restored, id)
        .xfer(&mut XferLoad::new(
            ObservedReader {
                cursor: Cursor::new(saved.clone()),
                events: restored_events.clone(),
            },
            7,
        ))
        .unwrap();
    assert_eq!(
        restored_events.sequence.load(Ordering::SeqCst),
        0x13,
        "native load applies conditions then dispatches changed decal before opacity channels"
    );
    assert_eq!(
        restored_events.decal_position.load(Ordering::SeqCst),
        explicit[0] as u64,
        "decal callback occurs immediately after enum, before explicit opacity read"
    );
    assert_eq!(payload(basic(&mut restored, id)), saved);
    let before_reload = restored_events.sequence.load(Ordering::SeqCst);
    basic(&mut restored, id)
        .xfer(&mut XferLoad::new(Cursor::new(&saved), 7))
        .unwrap();
    assert_eq!(
        restored_events.sequence.load(Ordering::SeqCst),
        (before_reload << 4) | 1,
        "equal-type reload still restores condition flags but emits no decal callback"
    );
    let mut particles = ParticleSystemManager::new();
    for frame in 1..=2 {
        step(basic(&mut source, id), &mut particles, frame);
        step(basic(&mut restored, id), &mut particles, frame);
        assert_eq!(fade(basic(&mut source, id)), fade(basic(&mut restored, id)));
        assert_eq!(
            payload(basic(&mut source, id)),
            payload(basic(&mut restored, id))
        );
    }
    assert_eq!(peer_events.sequence.load(Ordering::SeqCst), 0);
    assert_eq!(
        basic(&mut peer, peer_id).terrain_decal_type,
        TerrainDecalType::None
    );
}

#[test]
fn decal_target_exact_float_changes_preserve_cpp_rate_rule() {
    let mut first = BasicDrawable::new(DrawableId(9001));
    let peer = BasicDrawable::new(DrawableId(9001));
    first.set_terrain_decal_fade_target(0.5, 0.1);
    let adjacent = f32::from_bits(0.5f32.to_bits() + 1);
    first.set_terrain_decal_fade_target(adjacent, 0.2);
    assert_eq!(
        (
            first.decal_opacity_fade_target,
            first.decal_opacity_fade_rate
        ),
        (adjacent, 0.2),
        "distinct adjacent targets must change rate even within EPSILON"
    );
    first.set_terrain_decal_fade_target(adjacent, 0.7);
    assert_eq!(first.decal_opacity_fade_rate, 0.2);
    first.set_terrain_decal_fade_target(f32::NAN, 0.3);
    assert!(first.decal_opacity_fade_target.is_nan());
    first.set_terrain_decal_fade_target(f32::NAN, 0.4);
    assert_eq!(first.decal_opacity_fade_rate, 0.4);
    first.set_terrain_decal_fade_target(0., 0.5);
    first.set_terrain_decal_fade_target(-0., 0.6);
    assert_eq!(
        first.decal_opacity_fade_rate, 0.5,
        "signed zero targets compare equal"
    );
    assert_eq!(fade(&peer), (TerrainDecalType::None, 0., 0., 0.));
}
