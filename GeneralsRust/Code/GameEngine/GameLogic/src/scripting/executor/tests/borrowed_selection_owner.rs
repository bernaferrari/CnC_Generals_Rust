//! CPP1372 cache exercised through a real local selection producer.
use crate::commands::selection::{
    ObjectInfo, ObjectKind, ObjectLookup, PlayerSelection, SelectionType,
};
use crate::common::{Coord3D, IRegion2D, ObjectID};
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery, ScriptSelectionStatus};
use std::sync::Arc;

struct UnitLookup {
    id: ObjectID,
}
impl ObjectLookup for UnitLookup {
    fn get_object_info(&self, id: ObjectID) -> Option<ObjectInfo> {
        (id == self.id).then_some(ObjectInfo {
            id,
            drawable_id: None,
            position: Coord3D::new(0.0, 0.0, 0.0),
            owner_id: 0,
            kind: ObjectKind::Infantry,
            is_alive: true,
            is_selectable: true,
            is_controllable: true,
            is_crate: false,
            is_garrisonable_building: false,
        })
    }
    fn get_objects_in_region(&self, _: &IRegion2D) -> Vec<ObjectID> {
        vec![self.id]
    }
    fn get_all_objects(&self) -> Vec<ObjectID> {
        vec![self.id]
    }
    fn get_object_position(&self, id: ObjectID) -> Option<Coord3D> {
        self.get_object_info(id).map(|object| object.position)
    }
    fn is_object_alive(&self, id: ObjectID) -> bool {
        id == self.id
    }
    fn is_object_visible_to_player(&self, _: i32, id: ObjectID) -> bool {
        id == self.id
    }
    fn is_object_detected_by_player(&self, _: i32, id: ObjectID) -> bool {
        id == self.id
    }
    fn get_object_owner(&self, id: ObjectID) -> Option<i32> {
        (id == self.id).then_some(0)
    }
    fn get_object_kind(&self, id: ObjectID) -> Option<ObjectKind> {
        (id == self.id).then_some(ObjectKind::Infantry)
    }
    fn can_player_control(&self, player: i32, id: ObjectID) -> bool {
        player == 0 && id == self.id
    }
    fn set_object_selected(&self, _: ObjectID, _: bool) {} // Selection itself is the canonical UI state.
}

pub(super) struct SelectionOwner {
    selection: PlayerSelection,
    id: ObjectID,
    name: String,
    pub multiplayer: bool,
}
impl SelectionOwner {
    pub fn new(name: &str, id: ObjectID) -> Self {
        let mut selection = PlayerSelection::new(0);
        // Immutable definitions shared with the lookup interface need no mutex.
        selection.set_object_lookup(Arc::new(UnitLookup { id }));
        Self {
            selection,
            id,
            name: name.into(),
            multiplayer: false,
        }
    }
    pub fn select(&mut self, frame: u32) {
        self.selection.update(frame);
        assert!(
            self.selection
                .select_objects(vec![self.id], SelectionType::Add)
        );
    }
    pub fn clear(&mut self, frame: u32) {
        self.selection.update(frame);
        self.selection.clear_selection();
    }
}
impl ScriptExecutionDriver for SelectionOwner {
    fn after_action(&mut self) -> crate::GameLogicResult<()> {
        Ok(())
    }
    fn named_selection(&self, literal_name: &str) -> ScriptOwnerQuery<ScriptSelectionStatus> {
        ScriptOwnerQuery::Present(ScriptSelectionStatus {
            multiplayer: self.multiplayer,
            changed_frame: self.selection.get_last_selection_change(),
            selected: literal_name == self.name && self.selection.is_object_selected(self.id),
        })
    }
}
