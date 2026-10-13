//! TunnelTracker.cpp180-233: metadata loans end before immediate rider callbacks.
use super::*;
use crate::tunnel_tracker::TunnelRetirement;

impl BorrowedAiWorld<'_> {
    fn tunnel_player_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<Arc<RwLock<crate::player::Player>>, String> {
        let object = owner.try_read().map_err(|_| "tunnel owner borrowed")?;
        let actual = self.object(object.get_id()).ok_or("tunnel owner absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("foreign tunnel owner".into());
        }
        let controller = object
            .controlling_player_id_from_pin()?
            .ok_or("tunnel controller absent")?;
        self.players
            .get_player(controller as Int)
            .cloned()
            .ok_or("tunnel controller not supplied".into())
    }

    pub(crate) fn register_tunnel_owned(&mut self, id: ObjectID) -> Result<bool, String> {
        let owner = self.object(id).ok_or("tunnel creation owner absent")?;
        let contain = owner
            .try_read()
            .map_err(|_| "tunnel creation owner borrowed")?
            .get_contain();
        let Some(contain) = contain else {
            return Ok(false);
        };
        let need = contain
            .try_lock()
            .map_err(|_| "tunnel creation module borrowed")?
            .owned_tunnel_creation_needed();
        let Some(need) = need else {
            return Ok(false);
        };
        if !need {
            return Ok(true);
        }
        let player = self.tunnel_player_owned(&owner)?;
        {
            let mut player = player
                .try_write()
                .map_err(|_| "tunnel creation player borrowed")?;
            player.init_tunnel_tracker();
            let tracker = player
                .get_tunnel_system_mut()
                .ok_or("tunnel tracker absent")?;
            if tracker
                .get_container_list()
                .map_err(|error| error.to_string())?
                .contains(&id)
            {
                return Err("tunnel entrance registered twice".into());
            }
            tracker
                .on_tunnel_created_id(id)
                .map_err(|error| error.to_string())?;
        }
        contain
            .try_lock()
            .map_err(|_| "tunnel creation module borrowed")?
            .finish_owned_tunnel_creation(true);
        Ok(true)
    }

    pub(super) fn validate_tunnel_retirement_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<bool, String> {
        let (id, contain) = {
            let object = owner
                .try_read()
                .map_err(|_| "tunnel retirement owner borrowed")?;
            (object.get_id(), object.get_contain())
        };
        let Some(contain) = contain else {
            return Ok(false);
        };
        let registered = contain
            .try_lock()
            .map_err(|_| "tunnel retirement module borrowed")?
            .owned_tunnel_registered();
        let Some(registered) = registered else {
            return Ok(false);
        };
        if !registered {
            return Ok(true);
        }
        let player = self.tunnel_player_owned(owner)?;
        let (entrances, members) = {
            let player = player
                .try_read()
                .map_err(|_| "tunnel retirement player borrowed")?;
            let tracker = player
                .get_tunnel_system()
                .ok_or("registered tunnel tracker absent")?;
            tracker.validate_entrance_retirement(id)?;
            (
                tracker
                    .get_container_list()
                    .map_err(|error| error.to_string())?,
                tracker.get_contained_item_ids().to_vec(),
            )
        };
        for entrance in &entrances {
            let entrance = self
                .object(*entrance)
                .ok_or("registered tunnel entrance absent")?;
            if !Arc::ptr_eq(&player, &self.tunnel_player_owned(&entrance)?) {
                return Err("tunnel entrance belongs to another network".into());
            }
        }
        let mut visited = std::collections::BTreeSet::new();
        for id in members {
            if !visited.insert(id) {
                return Err("duplicate tunnel passenger".into());
            }
            let rider = self.object(id).ok_or("tunnel passenger absent")?;
            let object = rider.try_read().map_err(|_| "tunnel passenger borrowed")?;
            if !object
                .get_contained_by()
                .is_some_and(|parent| entrances.contains(&parent))
            {
                return Err("tunnel passenger backlink belongs to another network".into());
            }
            drop(object);
            if entrances.len() == 1 {
                self.validate_destroy_after_tunnel_detach(id)?;
            }
        }
        Ok(true)
    }

    pub(super) fn retire_tunnel_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<bool, String> {
        if !self.validate_tunnel_retirement_owned(owner)? {
            return Ok(false);
        }
        let (id, contain) = {
            let object = owner
                .try_read()
                .map_err(|_| "tunnel retirement owner borrowed")?;
            (
                object.get_id(),
                object.get_contain().ok_or("tunnel interface absent")?,
            )
        };
        if contain
            .try_lock()
            .map_err(|_| "tunnel retirement module borrowed")?
            .owned_tunnel_registered()
            != Some(true)
        {
            return Ok(true);
        }
        let player = self.tunnel_player_owned(owner)?;
        let retirement = {
            let mut player = player
                .try_write()
                .map_err(|_| "tunnel retirement player borrowed")?;
            player
                .get_tunnel_system_mut()
                .ok_or("tunnel tracker absent")?
                .retire_entrance_metadata(id)
        };
        let mut current = player
            .try_read()
            .map_err(|_| "tunnel cursor player borrowed")?
            .get_tunnel_system()
            .ok_or("tunnel tracker absent")?
            .get_contained_item_ids()
            .first()
            .copied();
        while let Some(rider_id) = current {
            current = player
                .try_read()
                .map_err(|_| "tunnel cursor player borrowed")?
                .get_tunnel_system()
                .ok_or("tunnel tracker absent")?
                .contained_successor(rider_id)?;
            let rider = self
                .object(rider_id)
                .ok_or("tunnel callback passenger absent")?;
            match retirement {
                TunnelRetirement::CaveIn => {
                    {
                        let mut object = rider.write().map_err(|_| "tunnel passenger poisoned")?;
                        object.finish_removed_from_local();
                        self.maintain_partition(&mut object, false);
                    }
                    self.destroy_object_owned(rider_id)?;
                }
                TunnelRetirement::Retarget(valid) => {
                    let mut object = rider.write().map_err(|_| "tunnel passenger poisoned")?;
                    if object.get_contained_by() == Some(id) {
                        object.admit_contained_by_at(valid, true, self.frame());
                        self.maintain_partition(&mut object, false);
                    }
                }
            }
        }
        if retirement == TunnelRetirement::CaveIn {
            player
                .try_write()
                .map_err(|_| "tunnel clear player borrowed")?
                .get_tunnel_system_mut()
                .ok_or("tunnel tracker absent")?
                .clear_caved_in_roster();
        }
        contain
            .try_lock()
            .map_err(|_| "tunnel retirement module borrowed")?
            .finish_owned_tunnel_retirement();
        Ok(true)
    }
}
