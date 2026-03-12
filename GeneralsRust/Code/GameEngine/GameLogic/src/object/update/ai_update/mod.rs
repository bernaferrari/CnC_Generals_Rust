//! Compatibility shims for Object/Update/AIUpdate modules.

pub mod assault_transport_ai_update;
pub mod chinook_ai_update;
pub mod deliver_payload_ai_update;
pub mod deliver_payload_data;
pub mod deploy_style_ai_update;
pub mod dozer_ai_update;
pub mod hack_internet_ai_update;
pub mod jet_ai_update;
pub mod missile_ai_update;
pub mod pow_truck_ai_update;
pub mod railed_transport_ai_update;
pub mod railroad_guide_ai_update;
pub mod supply_truck_ai_update;
pub mod transport_ai_update;
pub mod wander_ai_update;
pub mod worker_ai_update;

pub use assault_transport_ai_update::*;
pub use chinook_ai_update::*;
pub use deliver_payload_ai_update::*;
pub use deliver_payload_data::*;
pub use deploy_style_ai_update::*;
pub use dozer_ai_update::*;
pub use hack_internet_ai_update::*;
pub use jet_ai_update::*;
pub use missile_ai_update::*;
pub use pow_truck_ai_update::*;
pub use railed_transport_ai_update::*;
pub use railroad_guide_ai_update::*;
pub use supply_truck_ai_update::*;
pub use transport_ai_update::*;
pub use wander_ai_update::*;
pub use worker_ai_update::*;
