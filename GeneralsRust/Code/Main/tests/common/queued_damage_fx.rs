//! Observe the actual compiled native object-FX route. Sound names are metadata
//! read from the compiled nuggets; this does not claim device audio playback.
use game_client::fx_list::{FXList, FXNugget, FxViewServices};
use game_engine::common::ini::ini_fx_list;
use gamelogic::common::{Coord3D, Matrix3D};
use gamelogic::helpers::HostFxObjectPose;
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
        panic!("queued damage must retain the native object-FX route");
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

pub fn install(rows: &str) -> mpsc::Receiver<Observation> {
    let (events, receiver) = mpsc::channel();
    for level in ["Regular", "Veteran", "Elite"] {
        let name = format!("FX_{rows}_{level}");
        let definition = ini_fx_list::get_fx_list_store()
            .find_fx_list(&name)
            .expect("real Common INI parser admitted the authored fixture list")
            .clone();
        // These identities are unique across every fixture in this binary.
        // No reset or global observer replacement is involved.
        let compiled = FXList::from_definition(&definition);
        game_client::fx_list::with_fx_list_store_mut(|s| {
            s.add_fx_list(name.clone(), compiled);
        });
        let sounds = game_client::fx_list::sound_names_for_fx_list(&name);
        assert_eq!(sounds, vec![format!("Sound_{rows}_{level}")]);
        let compiled = game_client::fx_list::with_fx_list_store(|s| s.find_fx_list(&name)).unwrap();
        let mut observed = FXList::new();
        observed.add_fx_nugget(Box::new(ObservedList {
            list: name.clone(),
            compiled,
            sounds,
            events: events.clone(),
        }));
        game_client::fx_list::with_fx_list_store_mut(|s| {
            s.add_fx_list(name, observed);
        });
    }
    receiver
}

/// Production ObjectFxRuntime resolves lists from the receiving session's
/// catalog, not the thread-local client store `install` wrote into.
pub fn admit_into(world: &mut generals_main::game_logic::GameLogic) {
    let catalog = game_client::fx_list::with_fx_list_store(|s| s.clone());
    world.admit_receiving_fx_catalog(catalog).unwrap();
}

/// Test-thread owner (hq-2qedt.23): bind CommonServices / WorldServices /
/// MainServices on this thread before the fixture admits weapons, mirroring
/// the hq-2qedt.20 world-test helper.
pub fn bind_test_world() {
    if game_engine::common::common_services::try_common_services().is_none() {
        game_engine::common::common_services::install_common_services(Arc::new(
            game_engine::common::common_services::CommonServices::new(),
        ));
    }
    if gamelogic::system::engine_stores::try_active_services().is_none() {
        gamelogic::system::engine_stores::install_services(
            gamelogic::system::engine_stores::new_world_services(),
        );
    }
    if generals_main::main_services::try_main_services().is_none() {
        generals_main::main_services::install_main_services(Arc::new(
            generals_main::main_services::MainServices::new(),
        ));
    }
}
