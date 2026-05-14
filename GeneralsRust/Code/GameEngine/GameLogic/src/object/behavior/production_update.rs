//! ProductionUpdate - Unit/structure production management
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::{
    Bool, Int, ModuleData, ObjectID, UnsignedInt, MODELCONDITION_ACTIVELY_CONSTRUCTING,
};
use crate::helpers::TheGameLogic;
use crate::helpers::TheThingFactory;
use crate::modules::{
    BehaviorModuleInterface, ProductionUpdateInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use game_engine::common::system::{Snapshotable, Xfer};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct ProductionUpdateModuleData {
    pub base: BehaviorModuleData,
    pub max_queue_entries: Int,
    pub quantity_modifier: Vec<f32>,
}

impl Default for ProductionUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            max_queue_entries: 5,
            quantity_modifier: vec![1.0],
        }
    }
}

impl Snapshotable for ProductionUpdateModuleData {
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

crate::impl_legacy_module_data_via_base!(ProductionUpdateModuleData, base);

#[derive(Debug, Clone)]
pub struct ProductionQueueEntry {
    pub template_name: String,
    pub build_time: UnsignedInt,
    pub cost: Int,
}

pub struct ProductionUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<ProductionUpdateModuleData>,
    production_queue: VecDeque<ProductionQueueEntry>,
    current_entry: Option<ProductionQueueEntry>,
    current_production_start_frame: UnsignedInt,
    current_production_end_frame: UnsignedInt,
    current_production_total_frames: UnsignedInt,
    is_producing: Bool,
    /// When paused, saves the remaining production frames so resume can
    /// recalculate the target end frame from the current logic frame.
    paused_remaining_frames: UnsignedInt,
    /// True while production is paused — the update loop skips progress
    /// but keeps waking to avoid missing the resume signal.
    is_paused: Bool,
}

impl ProductionUpdate {
    fn sync_actively_constructing_flag(&self) {
        let should_set =
            self.is_producing || self.current_entry.is_some() || !self.production_queue.is_empty();
        let Some(object) = self.object.upgrade() else {
            return;
        };
        let Ok(mut guard) = object.write() else {
            return;
        };
        if should_set {
            guard.set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
        } else {
            guard.clear_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
        }
    }

    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<ProductionUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            production_queue: VecDeque::new(),
            current_entry: None,
            current_production_start_frame: 0,
            current_production_end_frame: 0,
            current_production_total_frames: 0,
            is_producing: false,
            paused_remaining_frames: 0,
            is_paused: false,
        })
    }

    pub fn queue_production(&mut self, entry: ProductionQueueEntry) -> Bool {
        if self.production_queue.len() >= self.module_data.max_queue_entries as usize {
            return false;
        }
        self.production_queue.push_back(entry);
        self.sync_actively_constructing_flag();
        true
    }

    pub fn cancel_production_entry(&mut self, index: usize) -> Bool {
        if index < self.production_queue.len() {
            self.production_queue.remove(index);
            self.sync_actively_constructing_flag();
            return true;
        }
        false
    }
}

impl UpdateModuleInterface for ProductionUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let current_frame = TheGameLogic::get_frame();

        if self.is_producing && !self.is_paused {
            if current_frame >= self.current_production_end_frame {
                if let Some(entry) = self.current_entry.take() {
                    if let Some(factory_object) = self.object.upgrade() {
                        if let Ok(factory_guard) = factory_object.read() {
                            let factory_pos = *factory_guard.get_position();
                            if let Some(team_arc) = factory_guard.get_team() {
                                if let Some(template) =
                                    TheThingFactory::find_template(entry.template_name.as_str())
                                {
                                    if let Ok(team_guard) = team_arc.read() {
                                        if let Ok(factory) = TheThingFactory::get() {
                                            if let Ok(new_object) =
                                                factory.new_object(template, &*team_guard)
                                            {
                                                if let Ok(mut new_guard) = new_object.write() {
                                                    let spawn_pos = crate::common::Coord3D::new(
                                                        factory_pos.x + 5.0,
                                                        factory_pos.y,
                                                        factory_pos.z,
                                                    );
                                                    let _ = new_guard.set_position(&spawn_pos);
                                                    new_guard.set_producer(Some(&*factory_guard));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                self.is_producing = false;
                self.sync_actively_constructing_flag();
            }
        }

        if !self.is_producing && !self.production_queue.is_empty() {
            if let Some(entry) = self.production_queue.pop_front() {
                self.current_production_start_frame = current_frame;
                self.current_production_total_frames = entry.build_time.max(1);
                self.current_production_end_frame =
                    current_frame + self.current_production_total_frames;
                self.current_entry = Some(entry);
                self.is_producing = true;
                self.sync_actively_constructing_flag();
            }
        }

        if self.is_producing {
            return UpdateSleepTime::Frames(10);
        }

        UpdateSleepTime::Forever
    }
}

impl BehaviorModuleInterface for ProductionUpdate {
    fn get_module_name(&self) -> &'static str {
        "ProductionUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
    fn get_production_update_interface(&mut self) -> Option<&mut dyn ProductionUpdateInterface> {
        Some(self)
    }
}

impl ProductionUpdateInterface for ProductionUpdate {
    fn can_produce(&self, template_name: &str) -> bool {
        if self.production_queue.len() >= self.module_data.max_queue_entries as usize {
            return false;
        }
        let Some(template) = crate::helpers::TheThingFactory::find_template(template_name) else {
            return false;
        };

        let parking_full = if let Some(object) = self.object.upgrade() {
            if let Ok(guard) = object.read() {
                guard
                    .with_parking_place_behavior(|parking_place| {
                        parking_place.should_reserve_door_when_queued(template.as_ref())
                            && !parking_place.has_available_space_for(template.as_ref())
                    })
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        };
        if parking_full {
            return false;
        }

        true
    }

    fn start_production(
        &mut self,
        template_name: String,
        _player_id: ObjectID,
    ) -> Result<(), String> {
        let template = crate::helpers::TheThingFactory::find_template(&template_name)
            .ok_or_else(|| format!("Missing template: {}", template_name))?;
        let build_time = template.calc_time_to_build(None).max(1) as UnsignedInt;
        let cost = template.calc_cost_to_build(None);
        let entry = ProductionQueueEntry {
            template_name,
            build_time,
            cost,
        };
        if self.queue_production(entry) {
            Ok(())
        } else {
            Err("Production queue full".to_string())
        }
    }

    fn cancel_production(&mut self, index: usize) -> Result<(), String> {
        if self.cancel_production_entry(index) {
            Ok(())
        } else {
            Err("Invalid production index".to_string())
        }
    }

    fn get_queue_size(&self) -> usize {
        self.production_queue.len()
    }

    fn get_production_progress(&self) -> f32 {
        if !self.is_producing {
            return 0.0;
        }
        let current_frame = TheGameLogic::get_frame();
        let elapsed = current_frame.saturating_sub(self.current_production_start_frame);
        let total = self.current_production_total_frames.max(1);
        (elapsed as f32 / total as f32).clamp(0.0, 1.0)
    }

    fn is_producing(&self) -> bool {
        self.is_producing
    }

    fn pause_production(&mut self) {
        if !self.is_producing || self.is_paused {
            return;
        }
        let current_frame = TheGameLogic::get_frame();
        self.paused_remaining_frames = self
            .current_production_end_frame
            .saturating_sub(current_frame);
        self.is_paused = true;
    }

    fn resume_production(&mut self) {
        if !self.is_paused {
            return;
        }
        let current_frame = TheGameLogic::get_frame();
        self.current_production_end_frame = current_frame.saturating_add(self.paused_remaining_frames);
        self.paused_remaining_frames = 0;
        self.is_paused = false;
    }

    fn set_hold_door_open(&mut self, _exit_door: usize, hold_it: bool) {
        if hold_it {
            if let Some(object) = self.object.upgrade() {
                if let Ok(mut guard) = object.write() {
                    guard.set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                }
            }
        }
    }
}

pub struct ProductionUpdateFactory;
impl ProductionUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(ProductionUpdate::new(thing, module_data)?))
    }
}
