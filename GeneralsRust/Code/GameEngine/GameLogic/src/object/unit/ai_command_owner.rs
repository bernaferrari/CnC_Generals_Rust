//! Owner access for synchronous AI command dispatch.

use super::identity::Unit;
use crate::common::{Coord3D, ObjectID};
use crate::object::Object;
use std::error::Error;
use std::sync::{Arc, RwLock, RwLockWriteGuard};

type CommandOwnerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Exact owner for one command operation. Native objects use their installed
/// Object handle directly; only the standalone compatibility route carries a
/// mutable Unit guard.
pub(super) enum CommandOwner<'a> {
    Object(Arc<RwLock<Object>>),
    LegacyUnit(RwLockWriteGuard<'a, Unit>),
}

impl CommandOwner<'_> {
    pub(super) fn base_arc(&self) -> Arc<RwLock<Object>> {
        match self {
            Self::Object(owner) => Arc::clone(owner),
            Self::LegacyUnit(unit) => unit
                .get_base_object()
                .expect("legacy command owner must retain its base Object"),
        }
    }

    pub(super) fn get_position(&self) -> CommandOwnerResult<Coord3D> {
        let owner = self.owner_arc()?;
        let guard = owner
            .read()
            .map_err(|_| command_owner_error("command owner Object lock poisoned"))?;
        Ok(*guard.get_position())
    }

    pub(super) fn get_id(&self) -> CommandOwnerResult<ObjectID> {
        let id = match self {
            Self::Object(owner) => owner
                .read()
                .map_err(|_| command_owner_error("command owner Object lock poisoned"))?
                .get_id(),
            Self::LegacyUnit(unit) => unit.get_id(),
        };
        Ok(id)
    }

    pub(super) fn forward_command_to_flight_deck(
        &self,
        command: &crate::ai::AiCommandParams,
    ) -> CommandOwnerResult<()> {
        let owner = self.owner_arc()?;
        let guard = owner
            .read()
            .map_err(|_| command_owner_error("command owner Object lock poisoned"))?;
        guard.forward_command_to_flight_deck(command);
        Ok(())
    }

    pub(super) fn legacy(&mut self) -> CommandOwnerResult<&mut Unit> {
        match self {
            Self::LegacyUnit(unit) => Ok(&mut **unit),
            Self::Object(_) => Err(command_owner_error(
                "native Object command owner cannot use a Unit-only fallback",
            )),
        }
    }

    fn owner_arc(&self) -> CommandOwnerResult<Arc<RwLock<Object>>> {
        match self {
            Self::Object(owner) => Ok(Arc::clone(owner)),
            Self::LegacyUnit(unit) => unit
                .get_base_object()
                .ok_or_else(|| command_owner_error("legacy command owner Object unavailable")),
        }
    }
}

fn command_owner_error(message: &'static str) -> Box<dyn Error + Send + Sync> {
    Box::new(std::io::Error::new(std::io::ErrorKind::Other, message))
}
