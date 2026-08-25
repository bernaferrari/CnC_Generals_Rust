//! Host `.sav` companion for `THE_W3D_GHOST_OBJECT_MANAGER`.
//!
//! C++ writes `CHUNK_GhostObject` (`GameState.cpp:305`) via Snapshotable xfer.
//! Host WorldSnapshot bincode cannot grow a positional field without a v9
//! mirror, so the chunk rides beside `CHUNK_GameLogic` and is stashed until
//! `restore_from_snapshot` installs it under `saveLockGhostObjects`.

use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::system::Snapshotable;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use gamelogic::object::w3d_ghost_object::{THE_W3D_GHOST_OBJECT_MANAGER, W3DGhostObjectManager};
use std::io::Cursor;
use std::sync::Mutex;

pub const CHUNK_GHOST_OBJECT: &str = "CHUNK_GhostObject";

static PENDING_GHOST_XFER: Mutex<Option<Vec<u8>>> = Mutex::new(None);

pub fn capture_w3d_ghost_xfer_bytes() -> SaveLoadResult<Vec<u8>> {
    let live = THE_W3D_GHOST_OBJECT_MANAGER
        .read()
        .map_err(|_| SaveLoadError::Corrupted("W3D ghost manager lock poisoned".into()))?;
    let mut manager = live.clone();
    drop(live);
    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut xfer = XferSave::new(cursor, 1);
        manager
            .xfer(&mut xfer)
            .map_err(SaveLoadError::Serialization)?;
    }
    Ok(bytes)
}

pub fn stash_loaded_w3d_ghost_xfer(bytes: Vec<u8>) {
    if let Ok(mut slot) = PENDING_GHOST_XFER.lock() {
        *slot = Some(bytes);
    }
}

pub fn take_loaded_w3d_ghost_xfer() -> Option<Vec<u8>> {
    PENDING_GHOST_XFER
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

pub fn restore_w3d_ghost_manager_from_xfer_bytes(bytes: &[u8]) -> SaveLoadResult<()> {
    let mut loaded = W3DGhostObjectManager::new();
    // C++ `GameState.cpp:661` locks creation until manager xfer unlocks
    // (`W3DGhostObject.cpp:1172`) and allocates the saved modules.
    loaded.set_save_lock_ghost_objects(true);
    {
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        loaded
            .xfer(&mut xfer)
            .map_err(SaveLoadError::Serialization)?;
    }
    let mut live = THE_W3D_GHOST_OBJECT_MANAGER
        .write()
        .map_err(|_| SaveLoadError::Corrupted("W3D ghost manager lock poisoned".into()))?;
    *live = loaded;
    Ok(())
}

pub fn save_lock_live_w3d_ghosts(locked: bool) -> SaveLoadResult<()> {
    let mut live = THE_W3D_GHOST_OBJECT_MANAGER
        .write()
        .map_err(|_| SaveLoadError::Corrupted("W3D ghost manager lock poisoned".into()))?;
    live.set_save_lock_ghost_objects(locked);
    Ok(())
}
