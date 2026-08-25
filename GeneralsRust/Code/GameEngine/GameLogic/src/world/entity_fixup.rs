//! Containment and producer reference fixup after envelope/snapshot restore.
//!
//! C++ `OpenContain::xfer` writes occupant ObjectIDs (`OpenContain.cpp:1574-1640`)
//! and `loadPostProcess` re-resolves them via `findObjectByID` (`:1749-1800`).
//! `Object::xfer` writes `m_producerID` (`Object.cpp:4049-4053`) and
//! `m_xferContainedByID` (`:4183-4197`); `Object::loadPostProcess` (`:4420-4425`)
//! rebinds `m_containedBy`. Missing targets fail closed here (empty + warn)
//! instead of C++ `DEBUG_CRASH`.

use super::GameWorld;
use super::entities::EntityId;
use super::entity_generation::EntityHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainFixup {
    pub container: EntityHandle,
    pub occupant: EntityHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProducerFixup {
    pub entity: EntityHandle,
    pub producer: Option<EntityHandle>,
}

impl GameWorld {
    pub fn apply_contain_producer_fixup(
        &mut self,
        contains: &[ContainFixup],
        producers: &[ProducerFixup],
    ) {
        self.contain_roster_mut().clear();
        for link in contains {
            if self.resolve_entity(link.container).is_none()
                || self.resolve_entity(link.occupant).is_none()
            {
                log::warn!(
                    "contain fixup orphan container={} occupant={}",
                    link.container.id().get(),
                    link.occupant.id().get()
                );
                continue;
            }
            let _ = self
                .contain_roster_mut()
                .enter(link.container.id(), link.occupant.id());
            if let Some(occupant) = self.world_mut().entity_mut(link.occupant.id()) {
                occupant.contained_by_host = link.container.id().get();
            }
        }
        for link in producers {
            if self.resolve_entity(link.entity).is_none() {
                log::warn!("producer fixup orphan entity={}", link.entity.id().get());
                continue;
            }
            let producer_raw = match link.producer {
                None => None,
                Some(producer) => {
                    if self.resolve_entity(producer).is_none() {
                        log::warn!(
                            "producer fixup orphan producer={} for entity={}",
                            producer.id().get(),
                            link.entity.id().get()
                        );
                        None
                    } else {
                        Some(producer.id().get())
                    }
                }
            };
            if let Some(entity) = self.world_mut().entity_mut(link.entity.id()) {
                entity.producer_id = producer_raw;
            }
        }
    }

    pub fn contain_occupants(&self, container: EntityId) -> &[EntityId] {
        self.contain_roster().occupants(container)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::entities::{TemplateRef, Transform};

    fn spawn_at(world: &mut GameWorld, raw: u32, health: f32) -> EntityHandle {
        let id = EntityId::from_raw(raw);
        world
            .spawn_entity_at(
                id,
                TemplateRef::new("Unit"),
                None,
                Transform::default(),
                health,
            )
            .expect("spawn_at");
        world.entity_handle(id).expect("handle")
    }

    #[test]
    fn contain_and_producer_relink_when_generations_match() {
        let mut world = GameWorld::new(1);
        let bunker = spawn_at(&mut world, 10, 200.0);
        let rider = spawn_at(&mut world, 11, 50.0);
        let factory = spawn_at(&mut world, 12, 300.0);
        let unit = spawn_at(&mut world, 13, 80.0);
        world.apply_contain_producer_fixup(
            &[ContainFixup {
                container: bunker,
                occupant: rider,
            }],
            &[ProducerFixup {
                entity: unit,
                producer: Some(factory),
            }],
        );
        assert_eq!(world.contain_occupants(bunker.id()), &[rider.id()]);
        assert_eq!(
            world.entity(rider.id()).expect("rider").contained_by_host,
            bunker.id().get()
        );
        assert_eq!(
            world.entity(unit.id()).expect("unit").producer_id,
            Some(factory.id().get())
        );
    }

    #[test]
    fn orphan_and_stale_generation_fail_closed() {
        let mut world = GameWorld::new(1);
        let bunker = spawn_at(&mut world, 20, 200.0);
        let rider = spawn_at(&mut world, 21, 50.0);
        let stale = EntityHandle::new(rider.id(), rider.generation().saturating_add(1));
        let missing = EntityHandle::new(EntityId::from_raw(99), 1);
        world.apply_contain_producer_fixup(
            &[
                ContainFixup {
                    container: bunker,
                    occupant: stale,
                },
                ContainFixup {
                    container: bunker,
                    occupant: missing,
                },
            ],
            &[ProducerFixup {
                entity: rider,
                producer: Some(missing),
            }],
        );
        assert!(world.contain_occupants(bunker.id()).is_empty());
        assert_eq!(world.entity(rider.id()).expect("rider").producer_id, None);
    }
}
