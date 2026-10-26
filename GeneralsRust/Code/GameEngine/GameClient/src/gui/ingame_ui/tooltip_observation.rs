//! Immutable facts for one receiving native hover operation; never a live mirror.
use std::collections::HashMap;
#[derive(Clone, Debug)]
pub struct MouseoverPlayerObservation {
    pub display_name: String,
    pub playable_side: bool,
    pub color: [u8; 4],
}
#[derive(Clone, Debug)]
pub struct MouseoverObjectObservation {
    pub visible_friendly: bool,
    pub player: Option<MouseoverPlayerObservation>,
    pub display_names: HashMap<String, String>,
}
#[derive(Clone, Debug, Default)]
pub struct ReceivingMouseoverFacts {
    pub multiplayer: bool,
    pub objects: HashMap<u32, MouseoverObjectObservation>,
}
impl ReceivingMouseoverFacts {
    pub(crate) fn object(&self, id: u32) -> &MouseoverObjectObservation {
        self.objects
            .get(&id)
            .expect("native hover admission validates every observed object")
    }
}
