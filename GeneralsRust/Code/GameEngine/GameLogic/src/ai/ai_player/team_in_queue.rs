//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

/// Team in the build/ready queue
#[derive(Debug)]
pub struct TeamInQueue {
    pub work_orders: Vec<WorkOrder>, // List of work orders for this team
    pub priority_build: bool,        // True if specifically requested
    pub team_name: Option<String>,   // Team that units go into
    /// C++ `TeamInQueue::m_team` — concrete team instance (not just name).
    pub team: Option<Arc<RwLock<crate::team::Team>>>,
    pub frame_started: u32,                 // Frame we started building
    pub sent_to_start_location: bool,       // Has team been sent to start location
    pub stop_queueing: bool,                // True to stop building new units
    pub reinforcement: bool,                // True if reinforcing existing team
    pub reinforcement_id: Option<ObjectID>, // Object being reinforced
}

impl TeamInQueue {
    pub fn new() -> Self {
        Self {
            work_orders: Vec::new(),
            priority_build: false,
            team_name: None,
            team: None,
            frame_started: 0,
            sent_to_start_location: false,
            stop_queueing: false,
            reinforcement: false,
            reinforcement_id: None,
        }
    }

    /// Returns true if all units in the team have finished building
    pub fn is_all_built(&self) -> bool {
        self.work_orders
            .iter()
            .all(|order| order.num_completed >= order.num_required)
    }

    /// Returns true if minimum required units have been built.
    ///
    /// C++ `TeamInQueue::isMinimumBuilt`: counts an assigned factory as +1 completed.
    pub fn is_minimum_built(&self) -> bool {
        for order in self.work_orders.iter().filter(|o| o.required) {
            let mut count = order.num_completed;
            if order.factory_id.is_some() {
                count += 1; // one currently building
            }
            if order.num_required > count {
                return false;
            }
        }
        true
    }

    /// C++ `TeamInQueue::includesADozer` — any work order KINDOF_DOZER.
    /// C++ `TeamInQueue::includesADozer`:
    /// KINDOF_DOZER and not a resource-gatherer work order (GLA workers are both).
    pub fn includes_a_dozer(&self) -> bool {
        self.work_orders.iter().any(|order| {
            // C++: isKindOf(DOZER) && !order->m_isResourceGatherer
            if order.is_resource_gatherer {
                return false;
            }
            if TheThingFactory::find_template(&order.thing_template)
                .map(|t| t.is_kind_of(KindOf::Dozer))
                .unwrap_or(false)
            {
                return true;
            }
            // Residual name heuristic when templates lack KindOf flags (unit tests /
            // early boot). Prefer "dozer"; "worker" only if no template was found.
            let n = order.thing_template.to_ascii_lowercase();
            if n.contains("dozer") {
                return true;
            }
            TheThingFactory::find_template(&order.thing_template).is_none() && n.contains("worker")
        })
    }

    /// Returns true if all factory builds are complete.
    ///
    /// C++ `TeamInQueue::areBuildsComplete`: true when no work order still has a factory.
    pub fn are_builds_complete(&self) -> bool {
        self.work_orders
            .iter()
            .all(|order| order.factory_id.is_none())
    }

    /// C++ `TeamInQueue::isBuildTimeExpired`.
    ///
    /// Uses team prototype `initial_idle_frames` as the build-time budget.
    /// `< 1` means unlimited (never expires).
    pub fn is_build_time_expired(&self) -> bool {
        // C++ uses m_team->getPrototype()->m_initialIdleFrames.
        let team_name = self
            .team
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|tg| tg.get_name().to_string()))
            .or_else(|| self.team_name.clone());
        let Some(team_name) = team_name else {
            return false;
        };
        let Ok(factory) = get_team_factory().lock() else {
            return false;
        };
        let Some(prototype) = factory.find_team_prototype(&team_name) else {
            return false;
        };
        let idle_frames = prototype.get_initial_idle_frames();
        if idle_frames < 1 {
            return false; // unlimited
        }
        let now = TheGameLogic::get_frame();
        now > self.frame_started.saturating_add(idle_frames as u32)
    }

    /// Disbands the team: transfers units to the default team, deletes non-singleton teams.
    ///
    /// Matches C++ AIPlayer.cpp:3554 TeamInQueue::disband.
    /// Prefers `m_team` handle; name lookup is fallback for legacy/xfer entries.
    pub fn disband(&mut self) -> Result<(), AiError> {
        let team_name = self.team_name.clone().unwrap_or_default();
        log::debug!("{} - team disbanded, build time expired.", team_name);

        // Prefer concrete m_team handle (C++); name lookup is fallback only.
        let team_arc = if let Some(arc) = self.team.clone() {
            arc
        } else if !team_name.is_empty() {
            let Ok(mut factory) = get_team_factory().lock() else {
                self.work_orders.clear();
                return Ok(());
            };
            let Some(arc) = factory.find_team(&team_name) else {
                self.work_orders.clear();
                return Ok(());
            };
            drop(factory);
            arc
        } else {
            self.work_orders.clear();
            return Ok(());
        };

        let Ok(mut team_guard) = team_arc.write() else {
            self.work_orders.clear();
            return Ok(());
        };

        let Some(controlling_player_id) = team_guard.get_controlling_player_id() else {
            self.work_orders.clear();
            return Ok(());
        };

        let default_team = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(controlling_player_id as i32).cloned())
            .and_then(|player_arc| player_arc.read().ok().and_then(|p| p.get_default_team()));

        let Some(default_team_arc) = default_team else {
            self.work_orders.clear();
            return Ok(());
        };

        if team_guard.get_id()
            == default_team_arc
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(0)
        {
            self.work_orders.clear();
            return Ok(());
        }

        let Ok(mut default_team_guard) = default_team_arc.write() else {
            self.work_orders.clear();
            return Ok(());
        };

        team_guard.transfer_units_to(&mut default_team_guard);

        // PARITY_NOTE: C++ calls m_team->deleteInstance() if !getIsSingleton().
        // In Rust, delete_team destroys all remaining members and marks the team for cleanup.
        // Since units were already transferred, the team should have no remaining members.
        if !(*team_guard).is_singleton() {
            team_guard.delete_team(false);
        }
        drop(team_guard);

        // C++ m_team = NULL after disband so ~TeamInQueue will not setActive.
        self.team = None;
        self.work_orders.clear();
        Ok(())
    }

    /// Stop queueing new units, just finish current ones
    pub fn stop_queueing(&mut self) {
        self.stop_queueing = true;
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        let mut work_order_count = self.work_orders.len() as u16;
        let _ = xfer.xfer_unsigned_short(&mut work_order_count);
        if xfer.is_loading() {
            self.work_orders.clear();
            for _ in 0..work_order_count {
                let mut order = WorkOrder::new(String::new());
                order.xfer(xfer);
                self.work_orders.push(order);
            }
        } else {
            for order in &mut self.work_orders {
                order.xfer(xfer);
            }
        }

        let mut priority_build = self.priority_build;
        let _ = xfer.xfer_bool(&mut priority_build);
        if xfer.is_loading() {
            self.priority_build = priority_build;
        }

        // C++: TeamID teamID = m_team ? m_team->getID() : TEAM_ID_INVALID;
        //      xferUser(&teamID); load: m_team = TheTeamFactory->findTeamByID(teamID);
        let mut team_id: u32 = self
            .team
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
            .unwrap_or(crate::team::TEAM_ID_INVALID);
        let _ = xfer.xfer_unsigned_int(&mut team_id);
        if xfer.is_loading() {
            if team_id == crate::team::TEAM_ID_INVALID {
                self.team = None;
                self.team_name = None;
            } else if let Ok(factory) = get_team_factory().lock() {
                if let Some(arc) = factory.find_team_by_id(team_id) {
                    self.team_name = arc.read().ok().map(|g| g.get_name().to_string());
                    self.team = Some(arc);
                } else {
                    self.team = None;
                    self.team_name = None;
                }
            }
        }

        let mut frame_started = self.frame_started as i32;
        let _ = xfer.xfer_int(&mut frame_started);
        if xfer.is_loading() {
            self.frame_started = frame_started as u32;
        }

        let mut sent_to_start_location = self.sent_to_start_location;
        let _ = xfer.xfer_bool(&mut sent_to_start_location);
        if xfer.is_loading() {
            self.sent_to_start_location = sent_to_start_location;
        }

        let mut stop_queueing = self.stop_queueing;
        let _ = xfer.xfer_bool(&mut stop_queueing);
        if xfer.is_loading() {
            self.stop_queueing = stop_queueing;
        }

        let mut reinforcement = self.reinforcement;
        let _ = xfer.xfer_bool(&mut reinforcement);
        if xfer.is_loading() {
            self.reinforcement = reinforcement;
        }

        let mut reinforcement_id = self.reinforcement_id.unwrap_or(INVALID_ID);
        let _ = xfer.xfer_object_id(&mut reinforcement_id);
        if xfer.is_loading() {
            self.reinforcement_id = if reinforcement_id == INVALID_ID {
                None
            } else {
                Some(reinforcement_id)
            };
        }
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) {
        let _ = xfer;
    }
}

/// C++ `~TeamInQueue`: if m_team remains, activate it (empty active teams are
/// cleaned up by Team). `disband` nulls the handle so Drop will not re-activate.
impl Drop for TeamInQueue {
    fn drop(&mut self) {
        if let Some(team_arc) = self.team.take() {
            if let Ok(mut tg) = team_arc.write() {
                tg.set_active();
            }
        }
    }
}
