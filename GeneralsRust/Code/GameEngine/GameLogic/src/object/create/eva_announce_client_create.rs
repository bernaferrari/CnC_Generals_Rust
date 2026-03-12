//! EvaAnnounceClientCreate module - Plays EVA announcement when object is created
//!
//! This is a client-side module not present in the original CreateModule list,
//! but retained for parity with the existing Rust integration.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::common::{audio::AudioEventRts, AsciiString, UnsignedInt};
use crate::helpers::{TheAudio, TheGameLogic};
use crate::object::create::{CreateModule, CreateModuleData};
use crate::player::{player_list, PLAYER_INDEX_INVALID};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, ModuleData, Thing as ThingTrait};

/// Data structure for EvaAnnounceClientCreate module
#[derive(Debug, Clone)]
pub struct EvaAnnounceClientCreateData {
    pub base: CreateModuleData,
    pub announce_event: Option<AsciiString>,
    pub enemy_only: bool,
    pub ally_only: bool,
    pub owner_only: bool,
}

impl EvaAnnounceClientCreateData {
    pub fn new() -> Self {
        Self {
            base: CreateModuleData::new(),
            announce_event: None,
            enemy_only: false,
            ally_only: false,
            owner_only: false,
        }
    }
}

impl Default for EvaAnnounceClientCreateData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for EvaAnnounceClientCreateData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        ModuleData::set_module_tag_name_key(&mut self.base, key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        ModuleData::get_module_tag_name_key(&self.base)
    }
}

impl Snapshotable for EvaAnnounceClientCreateData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// EvaAnnounceClientCreate module implementation
#[derive(Debug)]
pub struct EvaAnnounceClientCreate {
    base: CreateModule,
    module_data: Arc<EvaAnnounceClientCreateData>,
    announcement_played: AtomicBool,
}

impl EvaAnnounceClientCreate {
    pub fn new(thing: Arc<dyn ThingTrait>, module_data: Arc<EvaAnnounceClientCreateData>) -> Self {
        Self {
            base: CreateModule::new(thing),
            module_data,
            announcement_played: AtomicBool::new(false),
        }
    }

    fn should_announce_to_player(&self, player_index: usize, object_id: u32) -> bool {
        let Ok(list) = player_list().read() else {
            return true;
        };
        let Some(local_player) = list.get_player(player_index as i32) else {
            return true;
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return false;
        };
        let Ok(object_guard) = object_arc.read() else {
            return false;
        };

        if !object_guard.is_visible_to_player(player_index as UnsignedInt) {
            return false;
        }

        let Some(owner_player) = object_guard.get_controlling_player() else {
            return true;
        };
        let Ok(local_guard) = local_player.read() else {
            return true;
        };
        let Ok(owner_guard) = owner_player.read() else {
            return true;
        };

        if self.module_data.owner_only {
            return local_guard.get_player_index() == owner_guard.get_player_index();
        }
        if self.module_data.enemy_only {
            return local_guard.is_enemy_with_player(&*owner_guard);
        }
        if self.module_data.ally_only {
            return local_guard.is_allied_with_player(&*owner_guard);
        }

        true
    }
}

impl CreateInterface for EvaAnnounceClientCreate {
    fn on_create(&self) {}

    fn on_build_complete(&self) {
        if !self.base.should_do_on_build_complete() {
            return;
        }

        self.base.on_build_complete();

        if self.announcement_played.swap(true, Ordering::AcqRel) {
            return;
        }

        let Some(event) = &self.module_data.announce_event else {
            return;
        };

        let object_id = self
            .base
            .get_thing()
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or_default();
        if object_id == 0 {
            return;
        }

        let local_player_index = player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(PLAYER_INDEX_INVALID);

        if local_player_index == PLAYER_INDEX_INVALID {
            return;
        }

        if !self.should_announce_to_player(local_player_index as usize, object_id) {
            return;
        }

        if let Some(audio) = TheAudio::get() {
            let mut event_to_play = AudioEventRts::new(event.as_str());
            event_to_play.set_object_id(object_id);
            audio.add_audio_event(&event_to_play);
        }
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for EvaAnnounceClientCreate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}
