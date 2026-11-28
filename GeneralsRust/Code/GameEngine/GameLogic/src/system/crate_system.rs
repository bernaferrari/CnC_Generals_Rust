//! Legacy crate catalog and shared drop rules.

pub use crate::object::crate_system::{
    CrateSystem, get_crate_system, killed_by_type_mask_from_ini,
};
pub use generals_sim::crate_drop::{
    CrateCreationEntry, CrateDieEval, CrateDropPick, CrateTemplate, veterancy_level_from_ini_name,
};
