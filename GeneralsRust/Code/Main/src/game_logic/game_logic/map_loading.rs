//! The driving world's constructor admission phase. Never published or saved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum MapLoadingPhase {
    #[default]
    Runtime,
    NewMap,
    SavedMap,
    SavedSnapshot,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MapAttemptKind {
    New,
    Saved,
}
impl MapAttemptKind {
    pub(crate) fn is_new(self) -> bool {
        self == Self::New
    }
}
impl super::GameLogic {
    #[cfg(feature = "game_client")]
    pub(crate) fn require_native_drawable_owner(&mut self) {
        self.native_drawable_owner_required = true;
    }
    pub(crate) fn requires_native_drawable_owner(&self) -> bool {
        self.native_drawable_owner_required
    }

    pub(crate) fn begin_saved_snapshot_admission(&mut self) -> Result<(), String> {
        if self.map_loading_phase != MapLoadingPhase::Runtime {
            return Err("saved snapshot admission is already active".into());
        }
        self.map_loading_phase = MapLoadingPhase::SavedSnapshot;
        Ok(())
    }
    pub(crate) fn finish_saved_snapshot_admission(&mut self) -> Result<(), String> {
        if self.map_loading_phase != MapLoadingPhase::SavedSnapshot {
            return Err("saved snapshot admission has not completed its map foundation".into());
        }
        self.map_loading_phase = MapLoadingPhase::Runtime;
        Ok(())
    }
    pub(crate) fn begin_map_admission_attempt(&mut self) -> Result<MapAttemptKind, String> {
        let (next, kind) = match self.map_loading_phase {
            MapLoadingPhase::Runtime => (MapLoadingPhase::NewMap, MapAttemptKind::New),
            MapLoadingPhase::SavedSnapshot => (MapLoadingPhase::SavedMap, MapAttemptKind::Saved),
            _ => return Err("receiving map admission is already active".into()),
        };
        self.map_loading_phase = next;
        Ok(kind)
    }
    /// Called only after the receiving roster finished onLevelStart (new map),
    /// or after save map foundation (whose constructor suppression continues).
    pub(crate) fn finish_map_admission_attempt(
        &mut self,
        kind: MapAttemptKind,
    ) -> Result<(), String> {
        let expected = match kind {
            MapAttemptKind::New => MapLoadingPhase::NewMap,
            MapAttemptKind::Saved => MapLoadingPhase::SavedMap,
        };
        if self.map_loading_phase != expected {
            return Err("receiving map admission phase changed during its callbacks".into());
        }
        self.map_loading_phase = match kind {
            MapAttemptKind::New => MapLoadingPhase::Runtime,
            MapAttemptKind::Saved => MapLoadingPhase::SavedSnapshot,
        };
        Ok(())
    }
    pub(crate) fn is_loading_map(&self) -> bool {
        matches!(
            self.map_loading_phase,
            MapLoadingPhase::NewMap | MapLoadingPhase::SavedMap
        )
    }
    pub(crate) fn is_loading_saved_snapshot(&self) -> bool {
        matches!(
            self.map_loading_phase,
            MapLoadingPhase::SavedMap | MapLoadingPhase::SavedSnapshot
        )
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn native_drawable_birth_context(
        &self,
    ) -> game_client::core::game_client::NativeDrawableBirthContext {
        game_client::core::game_client::NativeDrawableBirthContext {
            loading_map: self.is_loading_map(),
            in_load_game: self.is_loading_saved_snapshot(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constructors_and_same_thread_peers_never_publish_or_change_loading_owner() {
        let mut a = super::super::GameLogic::new();
        assert!(!a.is_loading_map() && !a.is_loading_saved_snapshot());
        let kind = a.begin_map_admission_attempt().unwrap();
        let mut b = super::super::GameLogic::new();
        b.begin_saved_snapshot_admission().unwrap();
        let saved = b.begin_map_admission_attempt().unwrap();
        assert!(a.is_loading_map() && !a.is_loading_saved_snapshot());
        assert!(b.is_loading_map() && b.is_loading_saved_snapshot());
        b.finish_map_admission_attempt(saved).unwrap();
        assert!(!b.is_loading_map() && b.is_loading_saved_snapshot());
        drop(b);
        assert!(a.is_loading_map());
        a.finish_map_admission_attempt(kind).unwrap();
        assert!(!a.is_loading_map() && !a.is_loading_saved_snapshot());
    }
    #[test]
    fn map_abort_restores_save_phase_until_outer_companion_acceptance() {
        let mut world = super::super::GameLogic::new();
        world.begin_saved_snapshot_admission().unwrap();
        let map = world.begin_map_admission_attempt().unwrap();
        assert!(world.finish_saved_snapshot_admission().is_err());
        assert!(world.begin_map_admission_attempt().is_err());
        world.finish_map_admission_attempt(map).unwrap();
        assert!(world.is_loading_saved_snapshot());
        world.finish_saved_snapshot_admission().unwrap();
        assert!(!world.is_loading_saved_snapshot());
        assert!(world.finish_saved_snapshot_admission().is_err());
    }
}

#[cfg(all(test, feature = "game_client"))]
mod native_birth_observation_tests {
    #[test]
    fn birth_context_observes_both_driving_owner_guards_through_save_foundation() {
        let mut world = super::super::GameLogic::new();
        let birth = world.native_drawable_birth_context();
        assert!(!birth.loading_map && !birth.in_load_game);
        world.begin_saved_snapshot_admission().unwrap();
        let birth = world.native_drawable_birth_context();
        assert!(!birth.loading_map && birth.in_load_game);
        let map = world.begin_map_admission_attempt().unwrap();
        let birth = world.native_drawable_birth_context();
        assert!(birth.loading_map && birth.in_load_game);
        world.finish_map_admission_attempt(map).unwrap();
        let birth = world.native_drawable_birth_context();
        assert!(!birth.loading_map && birth.in_load_game);
        world.finish_saved_snapshot_admission().unwrap();
        let birth = world.native_drawable_birth_context();
        assert!(!birth.loading_map && !birth.in_load_game);
    }
}
