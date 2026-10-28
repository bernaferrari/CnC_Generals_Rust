//! Runtime-only attachment leases. An observation copy never borrows its
//! source Object's right to mutate a receiving world. No service is selected
//! here; GameLogic remains the owner and actual admission explicitly binds.
use std::sync::{Arc, Weak};

/// Capability for an Object's receiving Team factory. Like the other runtime
/// attachment, observation clones are inert until admission.
pub(crate) enum TeamFactoryRuntimeAttachment {
    Unbound,
    Attached(gamelogic::team::TeamFactoryHandle),
    DetachedClone,
}

impl Default for TeamFactoryRuntimeAttachment {
    fn default() -> Self {
        Self::Unbound
    }
}

impl Clone for TeamFactoryRuntimeAttachment {
    fn clone(&self) -> Self {
        Self::DetachedClone
    }
}

impl std::fmt::Debug for TeamFactoryRuntimeAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unbound => "Unbound",
            Self::Attached(_) => "Attached",
            Self::DetachedClone => "DetachedClone",
        })
    }
}

impl TeamFactoryRuntimeAttachment {
    pub(crate) fn bind(&mut self, owner: &gamelogic::team::TeamFactoryHandle) {
        *self = Self::Attached(owner.clone());
    }

    pub(crate) fn take_from(&mut self, original: &mut Self) {
        *self = std::mem::replace(original, Self::Unbound);
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::Unbound;
    }

    pub(crate) fn mark_entered_exited(&self, team_id: Option<u32>) {
        let (Self::Attached(factory), Some(team_id)) = (self, team_id) else {
            return;
        };
        factory.mark_team_entered_exited(team_id);
    }
}

pub(crate) enum RuntimeAttachment<T: ?Sized> {
    Unbound,
    Attached(Weak<T>),
    DetachedClone,
}
impl<T: ?Sized> Default for RuntimeAttachment<T> {
    fn default() -> Self {
        Self::Unbound
    }
}
impl<T: ?Sized> Clone for RuntimeAttachment<T> {
    fn clone(&self) -> Self {
        Self::DetachedClone
    }
}
impl<T: ?Sized> std::fmt::Debug for RuntimeAttachment<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unbound => "Unbound",
            Self::Attached(_) => "Attached",
            Self::DetachedClone => "DetachedClone",
        })
    }
}
impl<T: ?Sized> RuntimeAttachment<T> {
    pub(crate) fn bind(&mut self, owner: &Arc<T>) {
        *self = Self::Attached(Arc::downgrade(owner));
    }
    pub(crate) fn upgrade(&self) -> Option<Arc<T>> {
        match self {
            Self::Attached(owner) => owner.upgrade(),
            _ => None,
        }
    }
    pub(crate) fn ptr_eq(&self, other: &Weak<T>) -> bool {
        matches!(self, Self::Attached(owner) if owner.ptr_eq(other))
    }
    pub(crate) fn is_detached_clone(&self) -> bool {
        matches!(self, Self::DetachedClone)
    }
}
