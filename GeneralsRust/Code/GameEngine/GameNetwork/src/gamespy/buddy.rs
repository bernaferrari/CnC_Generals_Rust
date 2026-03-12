//! GameSpy Buddy System
//! Manages friend lists and buddy status

use crate::error::NetworkResult;
use std::collections::HashSet;
use tokio::sync::RwLock;
use tracing::info;

pub struct BuddySystem {
    buddies: RwLock<HashSet<String>>,
}

impl BuddySystem {
    pub async fn new() -> NetworkResult<Self> {
        Ok(Self {
            buddies: RwLock::new(HashSet::new()),
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

    pub async fn add_buddy(&self, buddy_id: String) -> NetworkResult<()> {
        let mut buddies = self.buddies.write().await;
        buddies.insert(buddy_id);
        Ok(())
    }

    pub async fn remove_buddy(&self, buddy_id: String) -> NetworkResult<()> {
        let mut buddies = self.buddies.write().await;
        buddies.remove(&buddy_id);
        Ok(())
    }

    pub async fn get_buddy_list(&self) -> HashSet<String> {
        let buddies = self.buddies.read().await;
        buddies.clone()
    }

    pub fn set_buddy_list(&mut self, buddies: HashSet<String>) {
        // This is a synchronous setter for internal use
        self.buddies = RwLock::new(buddies);
    }
}
