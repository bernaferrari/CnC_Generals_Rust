//! GameSpy Buddy System
//! Manages friend lists and buddy status

use crate::error::NetworkResult;
use std::collections::HashSet;
use tracing::info;

/// Buddy list owned by the GameSpy interface. Only the owning task's methods
/// touch it, so it is a plain field with no shared lock.
pub struct BuddySystem {
    buddies: HashSet<String>,
}

impl BuddySystem {
    pub async fn new() -> NetworkResult<Self> {
        Ok(Self {
            buddies: HashSet::new(),
        })
    }

    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Started buddy system");
        Ok(())
    }

    pub async fn stop(&mut self) -> NetworkResult<()> {
        info!("Stopped buddy system");
        Ok(())
    }

    pub async fn add_buddy(&mut self, buddy_id: String) -> NetworkResult<()> {
        self.buddies.insert(buddy_id);
        Ok(())
    }

    pub async fn remove_buddy(&mut self, buddy_id: String) -> NetworkResult<()> {
        self.buddies.remove(&buddy_id);
        Ok(())
    }

    pub async fn get_buddy_list(&self) -> HashSet<String> {
        self.buddies.clone()
    }

    pub fn set_buddy_list(&mut self, buddies: HashSet<String>) {
        // This is a synchronous setter for internal use
        self.buddies = buddies;
    }
}
