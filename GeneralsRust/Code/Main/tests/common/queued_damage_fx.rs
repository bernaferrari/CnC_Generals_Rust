//! Observe the actual compiled native object-FX route. Sound names are metadata
//! read from the compiled nuggets; this does not claim device audio playback.
use game_client::fx_list::{FXList, FXListStore, FXNugget, FxViewServices};
use generals_presentation::fx::HostFxObjectPose;
use generals_sim::weapon::{
    WeaponDefinition, WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
};
use glam::Mat4 as Matrix3D;
use glam::Vec3 as Coord3D;
use std::collections::HashMap;
use std::sync::{Arc, mpsc};

#[derive(Debug)]
pub struct Observation {
    pub list: String,
    pub victim: Option<u32>,
    pub source: Option<u32>,
    pub sounds: Vec<String>,
}
struct ObservedList {
    list: String,
    compiled: Arc<FXList>,
    sounds: Vec<String>,
    events: mpsc::Sender<Observation>,
}
impl ObservedList {
    fn report(&self, primary: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        self.events
            .send(Observation {
                list: self.list.clone(),
                victim: Some(primary.id),
                source: secondary.map(|pose| pose.id),
                sounds: self.sounds.clone(),
            })
            .expect("fixture owns its FX receiver through delivery");
    }
}
impl FXNugget for ObservedList {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        panic!("weapon damage must retain the native object-FX route");
    }
    fn do_fx_obj_host(&self, primary: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        self.compiled.do_fx_obj_host(primary, secondary);
        self.report(primary, secondary);
    }
    fn do_fx_obj_host_with_view(
        &self,
        primary: &HostFxObjectPose,
        secondary: Option<&HostFxObjectPose>,
        view: &mut FxViewServices<'_>,
    ) {
        // Preserve the receiving operation's exact services. Delegating through
        // the no-view adapter here would select an ambient tactical camera.
        self.compiled
            .do_fx_obj_host_with_view(primary, secondary, view);
        self.report(primary, secondary);
    }
}

/// Parse one authored weapon through the same parser as receiving startup.
pub fn parse_weapon_source(source: &str) -> (String, WeaponDefinition) {
    let mut definitions = HashMap::new();
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut definitions,
            source,
            WeaponDefinitionLoadType::Normal
        ),
        1,
    );
    definitions.into_iter().next().unwrap()
}

/// Compile and instrument fixture lists directly in the receiving world.
pub fn admit_observed_source(
    world: &mut generals_main::game_logic::GameLogic,
    source: &str,
    rows: &str,
) -> mpsc::Receiver<Observation> {
    let mut catalog = FXListStore::new();
    assert_eq!(catalog.apply_source(source).unwrap(), 3);
    let (events, receiver) = mpsc::channel();
    for level in ["Regular", "Veteran", "Elite"] {
        let name = format!("FX_{rows}_{level}");
        let sounds = catalog.sound_names(&name);
        assert_eq!(sounds, vec![format!("Sound_{rows}_{level}")]);
        let compiled = catalog.find_fx_list(&name).unwrap();
        let mut observed = FXList::new();
        observed.add_fx_nugget(Box::new(ObservedList {
            list: name.clone(),
            compiled,
            sounds,
            events: events.clone(),
        }));
        catalog.add_fx_list(name, observed);
    }
    world.admit_receiving_fx_catalog(catalog).unwrap();
    receiver
}
