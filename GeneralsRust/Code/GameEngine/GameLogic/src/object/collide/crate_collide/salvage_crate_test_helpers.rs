use crate::common::ObjectID;
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{PlayerList, player_list};
use std::sync::{Arc, RwLock};

/// Preserve services touched by the fixture, including on assertion failure.
pub(super) struct Services {
    players: Option<PlayerList>,
    objects: Vec<(ObjectID, Option<Arc<RwLock<Object>>>)>,
}

impl Services {
    pub(super) fn new() -> Self {
        Self {
            players: Some(std::mem::replace(
                &mut *player_list().write().expect("player list write"),
                PlayerList::new(),
            )),
            objects: Vec::new(),
        }
    }

    pub(super) fn register(&mut self, object: &Arc<RwLock<Object>>) {
        let id = object.read().expect("fixture object read").get_id();
        self.objects.push((id, OBJECT_REGISTRY.get_object(id)));
        OBJECT_REGISTRY.register_object(id, object);
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        for (id, previous) in self.objects.drain(..).rev() {
            OBJECT_REGISTRY.unregister_object(id);
            if let Some(object) = previous {
                OBJECT_REGISTRY.register_object(id, &object);
            }
        }
        *player_list().write().expect("restore player list") = self.players.take().unwrap();
    }
}
