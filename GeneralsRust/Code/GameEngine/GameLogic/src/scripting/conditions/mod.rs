//! Match-owned host script query snapshots and trigger edge state.
//!
//! C++ `ScriptConditions` evaluation lives in `scripting/evaluator`; this
//! module only keeps the host snapshot/trigger data those evaluators read.

mod helpers;

pub use helpers::{
    HostObjectTriggerPersist, HostScriptPlayerCensus, HostScriptQuery, HostScriptQueryObject,
    HostScriptQuerySnapshot, HostTechBuildingCensus, HostTriggerSlotPersist, HostTriggerWorld,
    ScriptSurfacesAllowed,
    host_object_did_enter, host_object_did_exit,
    host_object_has_kind, host_script_lookup_polygon_trigger,
};
