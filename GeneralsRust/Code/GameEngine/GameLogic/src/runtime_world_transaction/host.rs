use super::*;
use std::{marker::PhantomData, rc::Rc, sync::Arc};

/// Stages legacy team callbacks on the detached save candidate. The live
/// match retains its player, team and script state throughout preparation.
#[must_use = "keep the stage alive throughout candidate preparation"]
pub struct HostRuntimeWorldStage {
    candidate: Option<Arc<WorldServices>>,
    _thread_affinity: PhantomData<Rc<()>>,
}

impl HostRuntimeWorldStage {
    /// Start staging on the freshly constructed receiving world.
    pub fn begin(candidate: &Arc<WorldServices>) -> Self {
        assert!(
            candidate.team_factory().begin_save_stage(),
            "TeamFactory already staging a save"
        );
        Self {
            candidate: Some(Arc::clone(candidate)),
            _thread_affinity: PhantomData,
        }
    }

    /// Finish staging without publishing the candidate or executing callbacks.
    pub fn finish(mut self) -> StagedHostRuntimeWorld {
        let candidate = self.candidate.take().expect("missing save candidate");
        let team_factory_effects = candidate.team_factory().finish_save_stage();
        StagedHostRuntimeWorld {
            candidate,
            team_factory_effects,
            _thread_affinity: PhantomData,
        }
    }
}

impl Drop for HostRuntimeWorldStage {
    fn drop(&mut self) {
        if let Some(candidate) = self.candidate.take() {
            drop(candidate.team_factory().finish_save_stage());
        }
    }
}

/// Detached runtime after map and snapshot admission.
pub struct StagedHostRuntimeWorld {
    candidate: Arc<WorldServices>,
    team_factory_effects: Vec<TeamFactoryDeferredEffects>,
    _thread_affinity: PhantomData<Rc<()>>,
}

impl StagedHostRuntimeWorld {
    /// Check that Main is committing the matching world and that no legacy
    /// Core callbacks would escape native save admission.
    pub fn preflight_native(
        self,
        expected: &Arc<WorldServices>,
    ) -> Result<CheckedNativeRuntimeWorld, String> {
        if !Arc::ptr_eq(&self.candidate, expected) {
            return Err("native save stage belongs to a different world".into());
        }
        validate_native_team_effects(&self.team_factory_effects)?;
        Ok(CheckedNativeRuntimeWorld(self))
    }
}

/// Candidate which can be published at the host's checked commit boundary.
pub struct CheckedNativeRuntimeWorld(StagedHostRuntimeWorld);

impl CheckedNativeRuntimeWorld {
    /// Set the compatibility service binding at the existing native commit
    /// phase, immediately before the host installs the matching GameLogic.
    pub fn publish_services(self) {
        let candidate = self.0.candidate;
        if !engine_stores::is_services_active(&candidate) {
            engine_stores::install_services(candidate);
        }
    }
}

#[cfg(test)]
#[path = "host_candidate_lifetime_tests.rs"]
mod candidate_lifetime_tests;
