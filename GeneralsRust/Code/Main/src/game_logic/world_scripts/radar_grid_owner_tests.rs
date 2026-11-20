//! Retail RadarVanPing reveals and detects without an authored GridDecalTemplate.
use super::*;
use crate::game_logic::{
    NativeBodyVisualChange, NativeFactoryDrawableSource, NativeObjectRetirement,
    NativeProjectedRadiusAllocator, NativeSlaveSource, NativeViewEffects, Player,
    ScriptPresentationEffects,
};
use crate::presentation_frame::camera::build::SlaveCameraSample;
use game_client::core::game_client::{
    NativeDrawableBirthContext, NativeDrawableOwner, NativeDrawableWorld,
    NativeParticleRetirementService, ReceivingDrawablePolicy,
};
use game_engine::common::{
    ini::ini_game_data::CameraMovementRules,
    system::big_file_system::BigFileSystem,
    thing::{module::StaticGameLodLevel, module_factory::ModuleFactory},
};
use gamelogic::{common::Coord3D, system::map_loader::MapLoader};
use std::io::Read;

struct RadiusOwner<'a>(NativeDrawableOwner<'a>);
impl NativeViewEffects for RadiusOwner<'_> {
    fn projected_radius_allocator(&mut self) -> Option<&mut dyn NativeProjectedRadiusAllocator> {
        Some(&mut self.0)
    }
    fn local_player_id(&self) -> u32 {
        0
    }
    fn host_epoch(&self) -> u64 {
        1
    }
    fn sample_slave(&mut self, _: &NativeSlaveSource, _: &str) -> SlaveCameraSample {
        unreachable!("no camera attachment in RadarVanPing")
    }
    fn body_visual_changed(
        &mut self,
        _: &NativeBodyVisualChange,
        _: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        unreachable!("RadarVanPing does not change body visuals")
    }
    fn admit_factory_drawable(
        &mut self,
        source: NativeFactoryDrawableSource,
        birth: NativeDrawableBirthContext,
        content: &mut ModuleFactory,
    ) -> Result<(), String> {
        self.0
            .admit_semantic_drawable_with_content(
                source.drawable,
                birth,
                0,
                content,
                ReceivingDrawablePolicy {
                    time_of_day: Some(game_client::system::TimeOfDay::Afternoon),
                    use_draw_module_lod: true,
                    static_game_lod: StaticGameLodLevel::High,
                    use_tree_sway: true,
                },
                &mut NativeParticleRetirementService,
                |_| Ok(()),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    fn retire_physical_object_drawable(
        &mut self,
        _: &NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        unreachable!("this witness ends while RadarVanPing is live")
    }
}

fn archive_entry(files: &mut BigFileSystem, entry: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    files
        .open_file(entry, 0)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}

#[test]
#[ignore = "requires original Zero Hour INI and map archives"]
fn mounted_radar_scan_reveals_and_detects_without_a_projected_grid() {
    let content = crate::fixture_content::retail_content_root()
        .join("Command & Conquer Generals Zero Hour/Data");
    let (radar, movement, map) = {
        let _common = game_engine::common::common_services::CommonServicesScope::new();
        let mut ini = BigFileSystem::new();
        ini.open_archive_file(content.join("INIZH.big")).unwrap();
        let object_lists = archive_entry(&mut ini, "Data/INI/ObjectCreationList.ini");
        let object_lists = String::from_utf8_lossy(&object_lists);
        let scan_list = object_lists
            .split("ObjectCreationList SUPERWEAPON_RadarVanScan")
            .nth(1)
            .unwrap()
            .split("\nEnd")
            .next()
            .unwrap();
        assert!(scan_list.contains("ObjectNames = RadarVanPing"));
        let system_ini = archive_entry(&mut ini, "Data/INI/Object/System.ini");
        let system_ini = String::from_utf8_lossy(&system_ini);
        let radar = system_ini
            .split_once("Object RadarVanPing")
            .unwrap()
            .1
            .split_once("\nEnd")
            .unwrap()
            .0;
        assert!(radar.contains("DynamicShroudClearingRangeUpdate"));
        assert!(radar.contains("StealthDetectorUpdate"));
        assert!(!radar.contains("GridDecalTemplate"));
        let radar = format!("Object RadarVanPing{radar}\nEnd\n");
        let game_data = archive_entry(&mut ini, "Data/INI/GameData.ini");
        let movement = CameraMovementRules::default()
            .overlay_ini_text(&String::from_utf8_lossy(&game_data))
            .unwrap();
        let mut maps = BigFileSystem::new();
        maps.open_archive_file(content.join("MapsZH.big")).unwrap();
        let map_bytes = archive_entry(&mut maps, "Maps/Lone Eagle/Lone Eagle.map");
        let mut map = MapLoader::new();
        map.load_map_from_bytes(&map_bytes).unwrap();
        (radar, movement, map.to_map_data())
    };

    let mut world = GameLogic::new();
    let services = world.world_services.clone();
    gamelogic::system::engine_stores::with_world_services(&services, || {
        world
            .world_services
            .terrain()
            .write()
            .unwrap()
            .try_load_map_geometry(map)
            .unwrap();
        world.admit_camera_application_movement(movement);
        world.initialize_receiving_partition_grid().unwrap();
        world.add_player(Player::new(0, Team::GLA, "Scan owner", true));
        world.add_player(Player::new(1, Team::USA, "Detected enemy", false));
        assert_eq!(world.local_player_id(), Some(0));
        world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
            ("Data/INI/Object/System.ini".into(), radar),
            ("radar-owner-probe.ini".into(), "Object RadarOwnerProbe\n KindOf = IMMOBILE\n Body = ImmortalBody Body\n MaxHealth = 1\n InitialHealth = 1\n End\nEnd\n".into()),
            (
                "radar-target-probe.ini".into(),
                "Object RadarTargetProbe\n KindOf = INFANTRY\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = StealthUpdate Cloak\n InnateStealth = No\n GrantedBySpecialPower = Yes\n End\nEnd\n".into(),
            ),
        ]).unwrap()).unwrap();
        let source = world
            .create_object_for_player("RadarOwnerProbe", 0, Vec3::new(100.0, 0.0, 100.0))
            .unwrap();
        let center = Vec3::new(307.25, 0.0, 208.5);
        let target = world
            .create_object_for_player("RadarTargetProbe", 1, center + Vec3::X * 30.0)
            .unwrap();
        world
            .host_object_mut(target)
            .unwrap()
            .apply_grant_stealth(100);
        assert!(world.host_object(target).unwrap().status.stealthed);
        assert!(!world.host_object(target).unwrap().status.detected);
        let source_point = Coord3D::new(center.x, center.z, center.y);
        assert!(
            !services
                .shroud()
                .lock()
                .unwrap()
                .is_position_visible(0, &source_point)
        );

        let mut receiver = NativeDrawableWorld::default();
        let marker = receiver
            .borrow_native()
            .allocate_projected_radius("EXHorde", 20.0, [50.0, 50.0, 0.0], 0xff00_88ee, 0.6)
            .unwrap();
        world.frame = 100;
        {
            let mut view = RadiusOwner(receiver.borrow_native());
            let mut effects = ScriptPresentationEffects {
                view: Some(&mut view),
                ..Default::default()
            };
            assert!(world.activate_radar_scan_with_effects(
                0,
                Team::GLA,
                center,
                Some(source),
                &mut effects
            ));
            world
                .update_radar_van_pings_with_effects(&mut effects)
                .unwrap();
            world.update_radar_scans_with_effects(&mut effects);
            assert!(world.host_object(target).unwrap().detection_expires_frame > world.frame);
            world.frame += 1;
            world
                .update_stealth_and_detection_with_effects(&mut effects)
                .unwrap();
        }
        assert!(
            world
                .objects
                .registered()
                .any(|(_, object)| object.radar_van_ping && object.template_name == "RadarVanPing")
        );
        assert!(
            services
                .shroud()
                .lock()
                .unwrap()
                .is_position_visible(0, &source_point)
        );
        assert!(world.host_object(target).unwrap().status.detected);
        let rows = receiver.borrow_native().collect_projected_decal_items(true);
        let textures: Vec<_> = rows.iter().map(|item| item.texture_name.as_str()).collect();
        assert_eq!(textures, ["EXHorde"], "RadarVanPing never allocates EXGrid");
        marker.release();
        assert!(
            receiver
                .borrow_native()
                .collect_projected_decal_items(true)
                .is_empty()
        );
    });
}
