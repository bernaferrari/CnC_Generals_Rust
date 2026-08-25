//! Chinook AI Update Module (re-export to object/update/ai_update implementation).

pub use crate::object::update::chinook_ai_update::{
    ChinookAIUpdate, ChinookAIUpdateData, ChinookAIUpdateModuleData, ChinookFlightStatus,
    chinook_attack_allowed_by_kind_of, chinook_evac_and_exit_pipeline,
    chinook_evac_needs_takeoff_first, chinook_evac_pipeline, chinook_free_to_exit,
    chinook_move_to_bldg_arrived, chinook_move_to_bldg_preferred_height, chinook_should_auto_land,
    chinook_should_auto_takeoff,
};
