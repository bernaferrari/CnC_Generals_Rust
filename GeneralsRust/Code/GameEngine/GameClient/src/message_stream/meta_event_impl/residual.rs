// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

/// Wave 345: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    gamelogic::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Wave 981: host presentation drawable TOD residual (no OBJECT_REGISTRY).
/// Meta stores pending TOD; GameClient presentation shell drains + applies.
static HOST_PENDING_TOD_RESIDUAL: AtomicU8 = AtomicU8::new(0xFF);

/// Map meta TimeOfDay to residual tag (0xFF = none).
fn tod_residual_tag(tod: TimeOfDay) -> u8 {
    match tod {
        TimeOfDay::Morning => 0,
        TimeOfDay::Afternoon => 1,
        TimeOfDay::Evening => 2,
        TimeOfDay::Night => 3,
        TimeOfDay::Invalid => 0xFE,
    }
}

fn queue_host_drawable_tod_residual(time_of_day: TimeOfDay) {
    HOST_PENDING_TOD_RESIDUAL.store(tod_residual_tag(time_of_day), Ordering::SeqCst);
}

/// Drain pending host TOD residual for presentation shell apply.
pub fn take_host_drawable_tod_residual() -> Option<TimeOfDay> {
    let tag = HOST_PENDING_TOD_RESIDUAL.swap(0xFF, Ordering::SeqCst);
    match tag {
        0 => Some(TimeOfDay::Morning),
        1 => Some(TimeOfDay::Afternoon),
        2 => Some(TimeOfDay::Evening),
        3 => Some(TimeOfDay::Night),
        _ => None,
    }
}

/// Wave 988: host model-condition weather residual (bit0=pending, bit1=night, bit2=snow).
static HOST_PENDING_MODEL_COND_WEATHER_RESIDUAL: AtomicU8 = AtomicU8::new(0);

fn queue_host_model_condition_weather_residual(is_night: bool, is_snow: bool) {
    let mut tag = 0x1u8;
    if is_night {
        tag |= 0x2;
    }
    if is_snow {
        tag |= 0x4;
    }
    HOST_PENDING_MODEL_COND_WEATHER_RESIDUAL.store(tag, Ordering::SeqCst);
}

/// Drain pending host NIGHT/SNOW model-condition residual for presentation shell.
pub fn take_host_model_condition_weather_residual() -> Option<(bool, bool)> {
    let tag = HOST_PENDING_MODEL_COND_WEATHER_RESIDUAL.swap(0, Ordering::SeqCst);
    if tag & 0x1 == 0 {
        None
    } else {
        Some((tag & 0x2 != 0, tag & 0x4 != 0))
    }
}
