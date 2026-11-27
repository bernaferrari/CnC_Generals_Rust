//! Compatibility tests for the serialized object lifecycle inventory.

use super::entity_lifecycle_tags::INVENTORY_TAGS;
use crate::save_load::snapshot::lifecycle_envelope::{
    ENTITY_LIFECYCLE_ENVELOPE_VERSION, EntityLifecycleEnvelope, EntityModuleState,
};

const EXPECTED_INVENTORY_LEN: usize = 73;

pub(super) const EXPECTED_INVENTORY_TAGS: &[&str] = &[
    "StatusBits",
    "WeaponSlots",
    "TurretRuntime",
    "AltTurretRuntime",
    "UpgradeDie",
    "SpecialPowerCompletionDie",
    "BattleBusSlowDeath",
    "CaptureChannel",
    "HackerDisableChannel",
    "ToppleUpdate",
    "StructureToppleUpdate",
    "StructureCollapseUpdate",
    "KeepObjectDie",
    "WaveGuideUpdate",
    "BoneFXDamage",
    "PoisonedBehavior",
    "ObjectDefectionHelper",
    "FireWeaponPower",
    "FireWeaponWhenDamaged",
    "TransitionDamageFX",
    "FXListDie",
    "CreateObjectDie",
    "LifetimeUpdate",
    "SlowDeath",
    "HeightDieUpdate",
    "FuelAirGasSlowDeath",
    "NeutronMissileUpdate",
    "MissileLauncherBuildingUpdate",
    "ScudStormMissileFlight",
    "CarpetBombTransport",
    "ArtilleryBarrageTransport",
    "A10StrikeTransport",
    "DaisyCutterTransport",
    "AnthraxBombTransport",
    "ClusterMinesTransport",
    "EmpPulseTransport",
    "TensileFormationUpdate",
    "FireSpreadUpdate",
    "BaseRegenerateUpdate",
    "DefaultAutoHealBehavior",
    "EnemyNearUpdate",
    "AnimationSteeringUpdate",
    "FloatUpdate",
    "ProneUpdate",
    "RadiusDecalUpdate",
    "CheckpointUpdate",
    "SpectreGunshipDeployment",
    "SpectreGunshipUpdate",
    "SmartBombTargetHoming",
    "HelicopterSlowDeath",
    "JetSlowDeath",
    "MinefieldBehavior",
    "SpawnBehavior",
    "FiringTracker",
    "AssaultTransport",
    "DeployStyleAIUpdate",
    "SlavedUpdate",
    "CommandButtonHuntUpdate",
    "FireWeaponWhenDead",
    "CreateObjectDieTransfer",
    "SpecialPowerCooldown",
    "AuthoredSpecialPowerTimers",
    "WeaponLock",
    "EmoticonSurrender",
    "ProjectileFlightResiduals",
    "ProjectileFlightResiduals.v2",
    "ParachuteContainAttachments",
    "ActiveBody",
    "PhysicsBehavior",
    "RailroadBehavior",
    "RailedTransportDockUpdate",
    "PathExtraDistance",
    "OverlordAddon",
];

fn envelope_with_every_inventory_tag() -> EntityLifecycleEnvelope {
    EntityLifecycleEnvelope {
        version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
        entity_id: 3,
        destroyed: false,
        destroyed_at_frame: 0,
        module_states: INVENTORY_TAGS
            .iter()
            .map(|tag| EntityModuleState {
                tag: (*tag).to_string(),
                payload: vec![tag.len() as u8],
            })
            .chain(std::iter::once(EntityModuleState {
                tag: "UnknownFuture".to_string(),
                payload: vec![0xFF],
            }))
            .collect(),
    }
}

#[test]
fn inventory_len_and_declaration_order_are_locked() {
    assert_eq!(INVENTORY_TAGS.len(), EXPECTED_INVENTORY_LEN);
    assert_eq!(INVENTORY_TAGS, EXPECTED_INVENTORY_TAGS);
    assert_eq!(INVENTORY_TAGS[0], "StatusBits");
    assert_eq!(INVENTORY_TAGS[1], "WeaponSlots");
    assert_eq!(*INVENTORY_TAGS.last().expect("tags"), "OverlordAddon");
}

#[test]
fn snapshot_block_preserves_every_envelope_tag_including_unknown() {
    let envelope = envelope_with_every_inventory_tag();
    let bytes = super::encode_lifecycle_snapshot_block(std::slice::from_ref(&envelope));
    let decoded = super::decode_lifecycle_snapshot_block(&bytes).expect("decode saved lifecycle");
    assert_eq!(decoded.len(), 1);
    let taken = &decoded[0];
    assert_eq!(
        taken, &envelope,
        "known and unknown tag payloads must survive unchanged"
    );
    let tags: Vec<&str> = taken.module_states.iter().map(|m| m.tag.as_str()).collect();
    assert_eq!(tags.len(), EXPECTED_INVENTORY_LEN + 1);
    for (idx, expected) in INVENTORY_TAGS.iter().enumerate() {
        assert_eq!(
            tags[idx], *expected,
            "envelope tag {idx} dropped in the snapshot"
        );
    }
    assert_eq!(tags[EXPECTED_INVENTORY_LEN], "UnknownFuture");
}
