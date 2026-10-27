// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

/// Host path has no dual-world factory objects; presentation updates belong to
/// the receiving GameClient and are retained on its InGameUI owner.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    gamelogic::object::registry::OBJECT_REGISTRY.is_empty()
}
