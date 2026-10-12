//! One operation on canonical Physics, with a synchronous wake after its loan.
//! The borrowed adapter preserves classic callers; the installed adapter never
//! schedules while holding the ModuleEntry guard.
use super::*;

pub(crate) enum LocomotorPhysicsAccess<'a, 'storage: 'a> {
    Borrowed(&'a mut (dyn crate::modules::PhysicsBehavior + 'storage)),
    Installed {
        handle: &'a crate::object::PhysicsInterfaceHandle,
        frame: u32,
        wake: &'a mut dyn FnMut() -> Result<(), String>,
    },
}
impl LocomotorPhysicsAccess<'_, '_> {
    pub(super) fn with<R>(
        &mut self,
        operation: impl FnOnce(&mut dyn crate::modules::PhysicsBehavior) -> R,
    ) -> Result<R, String> {
        match self {
            Self::Borrowed(physics) => Ok(operation(*physics)),
            Self::Installed { handle, .. } => {
                let mut physics = handle
                    .access()
                    .map_err(|_| "native locomotor Physics poisoned")?;
                Ok(operation(&mut *physics))
            }
        }
    }
    pub(super) fn installed_is_stunned(&self) -> Result<Option<bool>, String> {
        match self {
            Self::Borrowed(_) => Ok(None),
            Self::Installed { handle, .. } => handle.installed_is_stunned().map(Some),
        }
    }
    pub(super) fn motive(
        &mut self,
        force: &Coord3D,
        owner: Option<&crate::object::Object>,
    ) -> Result<(), String> {
        match self {
            Self::Borrowed(physics) => {
                apply_motive_force_with_owner(*physics, force, owner);
                Ok(())
            }
            Self::Installed {
                handle,
                frame,
                wake,
            } => {
                let owner = owner.ok_or("native locomotor force owner absent")?;
                handle.apply_motive_force_with_wake(force, owner, *frame, &mut **wake)
            }
        }
    }
}

pub(crate) enum LocomotorPoseChange {
    Position(Coord3D),
    Orientation(Real),
}

pub(crate) struct LocomotorMovementObservations<'a> {
    pub(crate) frame: u32,
    pub(crate) gravity: Real,
    pub(crate) penalty: BodyDamageType,
    pub(crate) terrain: &'a crate::terrain::TerrainLogic,
    pub(crate) pathfinder: &'a crate::ai::Pathfinder,
    pub(crate) change_pose:
        &'a mut dyn FnMut(&mut crate::object::Object, LocomotorPoseChange) -> Result<(), String>,
}
impl LocomotorMovementObservations<'_> {
    pub(super) fn valid_terrain(
        &self,
        surfaces: u32,
        layer: crate::common::PathfindLayerEnum,
        position: Coord3D,
    ) -> bool {
        self.pathfinder.valid_movement_terrain(
            crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32),
            &LocomotorSet::from_surfaces(surfaces),
            &position,
        )
    }
    pub(super) fn height(&self, owner: &crate::object::Object) -> Real {
        Self::observe_height(owner, self.terrain)
    }

    pub(crate) fn observe_height(
        owner: &crate::object::Object,
        terrain: &crate::terrain::TerrainLogic,
    ) -> Real {
        let position = owner.get_position();
        let mut height = position.z
            - terrain.get_layer_height(
                position.x,
                position.y,
                crate::path::PathfindLayerEnum::from_u32(owner.get_layer() as u32),
                None,
                true,
            );
        if owner.test_status(crate::common::ObjectStatusTypes::DeckHeightOffset) {
            height -= owner.get_carrier_deck_height();
        }
        height
    }
}
