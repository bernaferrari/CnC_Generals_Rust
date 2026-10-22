//! Retail WeaponObjects.ini SCUDMissile authors no HeightDieUpdate.
//! Fixture normalizes trailing horizontal whitespace from the pinned entry.
//! INIZH.big SHA256 1a6d41a7a2cb31e67ad2f868aca9264ad069c275e0074f8a0d970a336071e9a0;
//! entry SHA256 aaa3aaecfa5e6b14584de07460d8ff8deeafac1a1c761884dc9a429efd0a27ad.
use super::super::*;

const SCUD_DEFINITION: &str = r#"Object SCUDMissile

  ; *** ART Parameters ***
  Draw = W3DModelDraw ModuleTag_01
    OkToChangeModelColor = Yes
    DefaultConditionState
      Model = UVScudLchr_M
    End
    ConditionState = REALLYDAMAGED
      Model = UVScudLchr_DM
    End
    ConditionState = JAMMED
      ParticleSysBone = None SparksMedium
    End
    ConditionState = REALLYDAMAGED JAMMED
      Model = UVScudLchr_DM
      ParticleSysBone = None SparksMedium
    End
  End

  ; ***DESIGN parameters ***
  DisplayName      = OBJECT:SCUDMissile
  EditorSorting   = SYSTEM
  VisionRange = 0.0
  ArmorSet
    Conditions      = None
    ;*Be careful with this type, use in conjunction with KindOf = BALLISTIC_MISSILE to restrict targeting
    ;*to select weapons, because ballistic missile armor is weak! Weapons capable of targeting must have
    ;*AntiBallisticMissile = Yes set in the Weapon.ini.
    Armor           = BallisticMissileArmor
    DamageFX        = None
  End

  ; *** ENGINEERING Parameters ***
  KindOf = PROJECTILE BALLISTIC_MISSILE
  Body = ActiveBody ModuleTag_02
    MaxHealth       = 100.0
    InitialHealth   = 100.0

    ; Subdual damage "Subdues" you (reaction defined by BodyModule) when it passes your max health.
    ; The cap limits how extra-subdued you can be, and the other numbers detemine how fast it drains away on its own.
    ; A projectile is not disabled, but instead loses target and scatters
    SubdualDamageCap = 200
    SubdualDamageHealRate = 100000
    SubdualDamageHealAmount = 50
  End

; ---- begin Projectile death behaviors
  Behavior = InstantDeathBehavior DeathModuleTag_01
    DeathTypes = NONE +DETONATED
    ; we detonated normally.
    ; no FX, just quiet destroy ourselves
  End
  Behavior = InstantDeathBehavior DeathModuleTag_02
    DeathTypes = NONE +LASERED
    ; shot down by laser.
    FX         = FX_GenericMissileDisintegrate
    OCL        = OCL_GenericMissileDisintegrate
  End
  Behavior = InstantDeathBehavior DeathModuleTag_03
    DeathTypes = ALL -LASERED -DETONATED
    ; shot down by nonlaser.
    FX         = FX_GenericMissileDeath
  End
; ---- end Projectile death behaviors

  Behavior = PhysicsBehavior ModuleTag_06
    Mass = 1
  End
  Behavior = MissileAIUpdate ModuleTag_07
    TryToFollowTarget = No
    FuelLifetime = 5000
    IgnitionDelay = 0
;   IgnitionFX = FX_ScudStormIgnition ;NOT WORKING
    InitialVelocity = 50                ; in dist/sec
    DistanceToTravelBeforeTurning = 200
    DistanceToTargetBeforeDiving = 200 ; When I hit this close to target, I ignore PreferredHeight.
  End
  Locomotor = SET_NORMAL SCUDMissileLocomotor

  Behavior = WeaponBonusUpgrade ModuleTag_08
    TriggeredBy = Upgrade_GLAAPRockets
  End

  Geometry = Cylinder ; Since collision extents can't tilt, this is a little fake
  GeometryMajorRadius = 4.0
  GeometryHeight = 4.0
  GeometryIsSmall = Yes

  Shadow = SHADOW_DECAL
End
"#;

#[test]
fn native_retail_scud_does_not_acquire_unrequested_height_module() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(SCUD_DEFINITION, "retail-scud-native.ini")
        .unwrap();
    let definition = parser.get_definition("SCUDMissile").unwrap();
    assert!(
        !definition
            .behavior_modules
            .iter()
            .any(|module| module.class_name.eq_ignore_ascii_case("HeightDieUpdate"))
    );
    let content = crate::assets::ReceivingObjectContent::from_sources([
        ("retail-scud-native.ini".into(), SCUD_DEFINITION.to_owned()),
        ("scud-launcher-owner.ini".into(), String::from(
            "Object NativeScudLauncherOwner\nKindOf = VEHICLE\nBody = ActiveBody LauncherBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n",
        )),
    ]).unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    let source = world
        .create_object("NativeScudLauncherOwner", Team::GLA, Vec3::ZERO)
        .unwrap();
    for toxin in [false, true] {
        let projectile = world
            .spawn_scud_launcher_missile_projectile(
                source,
                Vec3::ZERO,
                Vec3::new(300.0, 0.0, 0.0),
                None,
                toxin,
            )
            .unwrap();
        let owner = world.host_object(projectile).unwrap();
        assert!(owner.native_lifecycle.as_ref().unwrap().states.is_empty());
        assert!(owner.height_die.is_none());
        assert_eq!(owner.scud_launcher_missile_toxin, toxin);
        assert_eq!(owner.producer_id, Some(source));
        assert_eq!(owner.health.current, 100.0);
        world.tick_native_lifecycle(projectile);
        assert!(!world.host_object(projectile).unwrap().status.on_die_started);
    }
}
