//! Class phase is immutable scheduling metadata, including during callbacks.

use super::*;
use game_engine::common::thing::module::BaseModuleData;
use std::sync::mpsc;
use std::time::Duration;

struct PhaseModule {
    phase: SleepyUpdatePhase,
    data: Arc<BaseModuleData>,
}

impl EngineSnapshotable for PhaseModule {
    fn crc(&self, _xfer: &mut dyn EngineXfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn EngineXfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Module for PhaseModule {
    fn get_module_name_key(&self) -> NameKeyType {
        0
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn get_sleepy_update_interface(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_update_module_interface(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl UpdateModuleInterface for PhaseModule {
    fn get_update_phase(&self) -> SleepyUpdatePhase {
        self.phase
    }
}

fn installed(phase: SleepyUpdatePhase) -> (Object, UpdateModulePtr) {
    let data = Arc::new(BaseModuleData::new());
    Object::installed_update_proxy_for_test(
        0xA1_2A_0003,
        "ClassPhaseProbe",
        Box::new(PhaseModule {
            phase,
            data: data.clone(),
        }),
        data,
    )
}

#[test]
fn equal_object_ids_keep_each_installed_module_phase() {
    let _serial = crate::test_sync::lock();
    let (physics_object, physics) = installed(SleepyUpdatePhase::Physics);
    let (final_object, final_update) = installed(SleepyUpdatePhase::Final);
    assert_eq!(physics_object.get_id(), final_object.get_id());
    for _ in 0..3 {
        assert_eq!(
            physics.read().unwrap().get_update_phase(),
            SleepyUpdatePhase::Physics
        );
        assert_eq!(
            final_update.read().unwrap().get_update_phase(),
            SleepyUpdatePhase::Final
        );
    }
}

#[test]
fn installed_phase_is_readable_while_module_callback_holds_its_entry() {
    let _serial = crate::test_sync::lock();
    let (object, proxy) = installed(SleepyUpdatePhase::Physics);
    let index = *object.update_module_handles.last().unwrap();
    let entry = object.modules[index].clone();
    let (sender, receiver) = mpsc::channel();

    let (observed, worker) = entry.with_module(|_module| {
        // Start after entering the real module callback. The old proxy blocks
        // on this same entry, so the bounded receive expires. Release it and
        // join before asserting: RED must not hang or poison the module.
        let worker = std::thread::spawn(move || {
            let phase = proxy.read().unwrap().get_update_phase();
            sender.send(phase).unwrap();
        });
        let observed = receiver.recv_timeout(Duration::from_secs(2));
        (observed, worker)
    });
    worker
        .join()
        .expect("phase query worker finishes after callback");
    assert_eq!(
        observed,
        Ok(SleepyUpdatePhase::Physics),
        "a scheduler must read class metadata without re-entering a running module"
    );
}
