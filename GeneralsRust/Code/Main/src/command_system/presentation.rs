use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationTargetHint {
    pub id: ObjectId,
    pub is_alive: bool,
    pub is_structure: bool,
    pub is_resource: bool,
    pub under_construction: bool,
    pub sold: bool,
    pub team: crate::game_logic::Team,
    pub is_enemy_of_local: bool,
    pub is_neutral: bool,
    pub template_name: String,
    /// C++ normal `ACTIONTYPE_ENTER_OBJECT` capability/capacity result frozen
    /// from the presentation frame.  It is never inferred from the template
    /// spelling at physical-input time.
    pub can_be_entered: bool,
    /// Remaining frozen capacity for normal Enter.  This is passenger slots
    /// for TransportContain/RailedTransportContain and bodies for garrison or
    /// tunnel containers; the selected rider is checked against it before
    /// physical RMB emits the command.
    #[serde(default)]
    pub enter_available_capacity: usize,
    /// Whether `enter_available_capacity` is measured in the selected rider's
    /// authored TransportSlotCount rather than contained-body count.
    #[serde(default)]
    pub enter_uses_transport_slots: bool,
    /// Frozen `AllowInsideKindOf = INFANTRY` restriction for normal Enter.
    #[serde(default)]
    pub enter_requires_infantry: bool,
    /// Frozen `ForbidInsideKindOf = AIRCRAFT` restriction for normal Enter.
    #[serde(default)]
    pub enter_forbids_aircraft: bool,
    /// C++ `DISABLED_SUBDUED` closes container doors.
    #[serde(default)]
    pub enter_disabled_subdued: bool,
    /// Distinguishes a normal transport (whose allowed-roster slice is empty)
    /// from a RiderChangeContain whose roster was absent/unsupported.  This
    /// keeps old serialized frames fail-closed for RiderChange input.
    #[serde(default)]
    pub enter_is_rider_change: bool,
    /// Frozen RiderChange capability: physical RMB may emit Enter only when
    /// every selected source matches one of these exact authored RiderN
    /// template identities.  Full rider visual/weapon metadata is not copied
    /// into input and is revalidated by authority.
    #[serde(default)]
    pub rider_change_allowed_templates: Vec<String>,
    /// Wave 235: damaged structure/unit residual for repair/service classification.
    pub is_damaged: bool,
    /// Wave 235: ally of local player (same team).
    pub is_friendly_of_local: bool,
    /// Wave 235: structure provides vehicle/aircraft repair pad residual.
    pub provides_vehicle_repair: bool,
    /// Wave 235: structure provides aircraft repair residual.
    pub provides_aircraft_repair: bool,
    /// Wave 235: heal pad / medical residual.
    pub provides_heal: bool,
    /// C++ ActionManager rejects a service destination contained by another
    /// object. Freeze that fact with the same source tags; an older context
    /// defaults false rather than assuming the destination is usable.
    #[serde(default)]
    pub can_provide_service: bool,
    /// Exact source-authored DockUpdate family frozen from the target.  It is
    /// not derived from `KindOf`, containment, or template spelling.
    #[serde(default)]
    pub dock_kind: crate::game_logic::DockKind,
    /// Frozen C++ controlling-player equality for SupplyCenter Dock.  This is
    /// deliberately narrower than `is_friendly_of_local`: allied players may
    /// use the same faction but cannot deposit into each other's center.
    /// Ownerless legacy frames set it only through their unambiguous
    /// faction-wide fallback.
    #[serde(default)]
    pub dock_controller_is_local: bool,
    /// Warehouse remaining cash/boxes represented in host supply units.
    #[serde(default)]
    pub stored_supplies: u32,
    /// Exact C++ `KINDOF_CAPTURABLE` frozen from Object INI metadata.
    #[serde(default)]
    pub capturable: bool,
    /// Exact C++ `KINDOF_IMMUNE_TO_CAPTURE` frozen from Object INI metadata.
    #[serde(default)]
    pub immune_to_capture: bool,
    /// Exact `GarrisonContain` presence; ordinary Enter capacity does not
    /// imply this capture-specific semantic.
    #[serde(default)]
    pub capture_garrisonable: bool,
    /// Number of target garrison occupants that are not stealthed.  This is
    /// frozen from the same presentation frame and fails closed on a missing
    /// contained record.
    #[serde(default)]
    pub capture_nonstealthed_garrison_count: u16,
    /// Number of target occupants allied to the local player, used for C++
    /// `appearsToContainFriendlies` parity during physical RMB classification.
    #[serde(default)]
    pub capture_friendly_garrison_count: u16,
    /// C++ rejects an undetected pure-stealth target before capture.  A
    /// disguised target is not represented as true here.
    #[serde(default)]
    pub capture_target_effectively_stealthed: bool,
    /// C++ `KINDOF_CRATE` frozen for ordinary crate pickup (MSG_DO_MOVETO).
    #[serde(default)]
    pub is_crate: bool,
    /// C++ `Object::isSalvageCrate` frozen for MSG_DO_SALVAGE.
    #[serde(default)]
    pub is_salvage_crate: bool,
    /// C++ `KINDOF_VEHICLE` frozen for hijack / convert-to-carbomb.
    #[serde(default)]
    pub is_vehicle: bool,
    /// C++ `KINDOF_AIRCRAFT` — hijack/carbomb reject airborne vehicles.
    #[serde(default)]
    pub is_aircraft: bool,
    /// C++ `KINDOF_DRONE` — `canHijackVehicle` rejects drones.
    #[serde(default)]
    pub is_drone: bool,
    /// C++ `OBJECT_STATUS_IS_CARBOMB` — convert-to-carbomb rejects already-bombs.
    #[serde(default)]
    pub is_carbomb: bool,
}

/// Wave 229: presentation-frozen selected-unit capability for RMB classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationSelectedUnitHint {
    pub id: ObjectId,
    pub is_alive: bool,
    /// C++ `KINDOF_HARVESTER`, frozen from the presentation snapshot for
    /// resource Gather classification.  Missing data must not grant gather
    /// permission when loading an older serialized input context.
    #[serde(default)]
    pub is_resource_collector: bool,
    /// Legacy builder/worker capability used by construction and repair
    /// classification.  It is deliberately not used for resource Gather.
    pub is_worker: bool,
    pub can_attack: bool,
    pub can_move: bool,
    /// C++ service actions require a source that is not contained. This is
    /// intentionally distinct from generic movement, which can be retained
    /// by a container/rider presentation record.
    #[serde(default)]
    pub can_request_service: bool,
    pub can_capture: bool,
    pub template_name: String,
    /// Wave 235: dozer/worker repair residual.
    pub can_repair: bool,
    /// Wave 235: damaged unit residual (seek repair/heal).
    pub is_damaged: bool,
    pub is_vehicle: bool,
    pub is_aircraft: bool,
    /// C++ `Object::isAboveTerrain`, frozen from the same presentation frame
    /// as an aircraft repair click. Older serialized contexts default false
    /// so they cannot fabricate an airfield landing request.
    #[serde(default)]
    pub is_above_terrain: bool,
    pub is_infantry: bool,
    /// C++ Object INI `TransportSlotCount`, frozen for normal Enter.  A
    /// missing/zero value must not board a capacity-checked transport.
    #[serde(default)]
    pub transport_slot_count: usize,
    /// Carried supply value frozen so SupplyCenter Dock can require a nonempty
    /// collector without a live authority read during physical RMB handling.
    #[serde(default)]
    pub stored_supplies: u32,
    /// Exact local-player control frozen with the selected object.  A physical
    /// RMB context must never turn a foreign/allied object into a SupplyCenter
    /// depositor merely because the faction tint is friendly.
    #[serde(default)]
    pub is_controlled_by_local: bool,
    /// Exact Object INI SpecialAbility capture module (not a unit name).
    #[serde(default)]
    pub capture_power: crate::game_logic::CapturePowerKind,
    /// Frozen `SpecialPowerModule::isReady` for that exact capture module.
    #[serde(default)]
    pub capture_power_ready: bool,
    /// C++ `KINDOF_SALVAGER` frozen for MSG_DO_SALVAGE classification.
    #[serde(default)]
    pub is_salvager: bool,
    /// C++ `findSpecialPowerWithOverridableDestinationActive` residual.
    #[serde(default)]
    pub can_override_special_power_destination: bool,
}

/// Information needed for command creation from mouse input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseCommandContext {
    pub world_position: Vec3,
    pub target_object: Option<ObjectId>,
    /// Presentation freeze for target classification (InGame).
    pub target_presentation: Option<PresentationTargetHint>,
    /// Wave 229: presentation freeze for selected-unit capabilities (InGame).
    pub selected_presentation: Vec<PresentationSelectedUnitHint>,
    /// Wave 236: presentation-frozen box-select unit ids (drag LMB).
    #[serde(default)]
    pub presentation_box_select_units: Vec<ObjectId>,
    /// Wave 236: presentation-frozen select-similar unit ids (double-click LMB).
    #[serde(default)]
    pub presentation_select_similar_units: Vec<ObjectId>,
    pub screen_position: Vec2,
    pub viewport_size: Option<Vec2>,
    pub world_min: Option<Vec3>,
    pub world_max: Option<Vec3>,
    pub mouse_button: MouseButton,
    pub modifier_keys: ModifierKeys,
    pub is_drag: bool,
    pub drag_start: Option<Vec2>,
    pub drag_end: Option<Vec2>,
    pub drag_start_world: Option<Vec3>,
    pub drag_end_world: Option<Vec3>,
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Command system state for tracking mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandMode {
    Normal,
    ForceAttack,
    ForceMove,
    Waypoint,
    BuildMode { template_name: String },
    SpecialPower { power_type: SpecialPowerType },
}
