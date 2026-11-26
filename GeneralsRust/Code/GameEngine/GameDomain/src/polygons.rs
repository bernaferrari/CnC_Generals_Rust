//! Authored polygon queries used by the simulation.

/// A polygon descriptor with a stable identity in its receiving world.
pub trait TriggerArea: Clone {
    fn trigger_id(&self) -> i32;
    fn trigger_name(&self) -> &str;
    fn contains_trigger_point(&self, x: i32, y: i32) -> bool;
}
