use super::*;

impl PathfindingSystem {
    pub(crate) fn object_uses_aircraft_goal_reservations(object_id: ObjectID) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if object_id == INVALID_ID {
            return false;
        }
        OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                let Some(ai) = obj_guard.get_ai_update_interface() else {
                    return false;
                };
                ai.lock()
                    .ok()
                    .map(|ai_guard| ai_guard.is_aircraft_that_adjusts_destination())
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    pub(crate) fn destination_only_result(
        from: Coord3D,
        to: Coord3D,
        layer: PathfindLayerEnum,
    ) -> PathResult {
        let mut waypoints = Vec::with_capacity(2);
        let mut layers = Vec::with_capacity(2);
        if (from.x - to.x).abs() > f32::EPSILON
            || (from.y - to.y).abs() > f32::EPSILON
            || (from.z - to.z).abs() > f32::EPSILON
        {
            waypoints.push(from);
            layers.push(layer);
        }
        waypoints.push(to);
        layers.push(layer);
        PathResult {
            success: true,
            waypoints,
            layers,
            total_cost: 0,
            can_optimize: Vec::new(),
            blocked_by_ally: false,
        }
    }

    pub fn new(width: usize, height: usize) -> Self {
        Self {
            pathfinder: Arc::new(Mutex::new(AStarPathfinder::new(width, height))),
            optimizer: PathOptimizer::new(),
            bridges: Vec::new(),
            request_queue: Arc::new(Mutex::new(VecDeque::new())),
            object_path_queue: Arc::new(Mutex::new(ObjectPathQueue::new())),
            goal_cells: Arc::new(Mutex::new(vec![vec![GoalCell::new(); height]; width])),
            path_cache: Arc::new(Mutex::new(HashMap::new())),
            zones: Arc::new(Mutex::new(ZoneManager::new(width, height))),
            width,
            height,
            is_map_ready: false,
            unit_goal_cells: Arc::new(Mutex::new(HashMap::new())),
            unit_pos_cells: Arc::new(Mutex::new(HashMap::new())),
            wall_pieces: Vec::new(),
            wall_cells: Arc::new(Mutex::new(HashSet::new())),
            is_tunneling: false,
            ignore_obstacle_id: INVALID_ID,
            wall_height: 0.0,
            cumulative_cells_allocated: AtomicI32::new(0),
            move_allies_depth: 0,
            open_list_count: 0,
            closed_list_count: 0,
            extent_lo: ICoord2D::new(0, 0),
            extent_hi: ICoord2D::new(0, 0),
            logical_extent_lo: ICoord2D::new(0, 0),
            logical_extent_hi: ICoord2D::new(0, 0),
            debug_path: None,
            debug_path_pos: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    /// Reset pathfinding state for a new map.
    /// C++ `Pathfinder::reset` (AIPathfind.cpp:3816-3880).
    pub fn reset(&mut self) {
        if let Ok(mut queue) = self.request_queue.lock() {
            queue.clear();
        }
        if let Ok(mut cache) = self.path_cache.lock() {
            cache.clear();
        }
        if let Ok(mut goals) = self.goal_cells.lock() {
            for row in goals.iter_mut() {
                for cell in row.iter_mut() {
                    *cell = GoalCell::new();
                }
            }
        }
        if let Ok(mut zones) = self.zones.lock() {
            zones.reset();
        }
        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.reset();
        }
        self.bridges.clear();
        if let Ok(mut oq) = self.object_path_queue.lock() {
            *oq = ObjectPathQueue::new();
        }
        if let Ok(mut ug) = self.unit_goal_cells.lock() {
            ug.clear();
        }
        if let Ok(mut up) = self.unit_pos_cells.lock() {
            up.clear();
        }
        self.wall_pieces.clear();
        if let Ok(mut walls) = self.wall_cells.lock() {
            walls.clear();
        }
        self.extent_lo = ICoord2D::new(0, 0);
        self.extent_hi = ICoord2D::new(0, 0);
        self.logical_extent_lo = ICoord2D::new(0, 0);
        self.logical_extent_hi = ICoord2D::new(0, 0);
        self.ignore_obstacle_id = INVALID_ID;
        self.is_tunneling = false;
        self.move_allies_depth = 0;
        self.is_map_ready = false;
        self.cumulative_cells_allocated.store(0, Ordering::Relaxed);
        self.open_list_count = 0;
        self.closed_list_count = 0;
        self.wall_height = 0.0;
        self.debug_path = None;
        self.debug_path_pos = Coord3D::new(0.0, 0.0, 0.0);
    }

    /// Queue a pathfinding request (full request residual).
    /// Also enqueues `object_id` into the C++ ObjectID ring when non-invalid.
    pub fn queue_path_request(&self, request: PathRequest) -> Result<(), String> {
        if request.object_id != INVALID_ID {
            let mut oq = self.object_path_queue.lock().unwrap();
            if !oq.queue(request.object_id) {
                return Err("Pathfind queue full".to_string());
            }
        }
        let mut queue = self.request_queue.lock().unwrap();
        if queue.iter().any(|r| r.object_id == request.object_id) {
            return Ok(());
        }
        if queue.len() >= PATHFIND_QUEUE_LEN {
            return Err("Pathfind queue full".to_string());
        }
        queue.push_back(request);
        Ok(())
    }

    /// C++ `Pathfinder::queueForPath(ObjectID)` — ring buffer of object ids.
    pub fn queue_for_path(&self, object_id: ObjectID) -> bool {
        let Ok(mut oq) = self.object_path_queue.lock() else {
            return false;
        };
        oq.queue(object_id)
    }

    /// C++ `Pathfinder::processPathfindQueue` (AIPathfind.cpp:5857-5938).
    ///
    /// Recalculates zones when dirty, then drains ObjectID ring until empty or
    /// PATHFIND_CELLS_PER_FRAME budget (C++ m_cumulativeCellsAllocated).
    /// C++ `m_logicalExtent` refresh from terrain (AIPathfind.cpp:5887-5897).
    pub fn refresh_logical_extent(&mut self) {
        // C++: TheTerrainLogic->getExtent → floor(/PATHFIND_CELL_SIZE_F); hi--.
        let (lo, hi) = if let Some(terrain) = TheTerrainLogic::get() {
            let ext = terrain.get_extent();
            let mut lo_x = (ext.lo.x / PATHFIND_CELL_SIZE_F).floor() as i32;
            let mut lo_y = (ext.lo.y / PATHFIND_CELL_SIZE_F).floor() as i32;
            let mut hi_x = (ext.hi.x / PATHFIND_CELL_SIZE_F).floor() as i32;
            let mut hi_y = (ext.hi.y / PATHFIND_CELL_SIZE_F).floor() as i32;
            hi_x -= 1;
            hi_y -= 1;
            // Clamp to pathfind map.
            lo_x = lo_x.max(0);
            lo_y = lo_y.max(0);
            hi_x = hi_x.min(self.width.saturating_sub(1) as i32).max(lo_x);
            hi_y = hi_y.min(self.height.saturating_sub(1) as i32).max(lo_y);
            (ICoord2D::new(lo_x, lo_y), ICoord2D::new(hi_x, hi_y))
        } else {
            (
                ICoord2D::new(0, 0),
                ICoord2D::new(
                    self.width.saturating_sub(1) as i32,
                    self.height.saturating_sub(1) as i32,
                ),
            )
        };
        self.logical_extent_lo = lo;
        self.logical_extent_hi = hi;
    }

    /// C++ human logical-map clamp.
    #[inline]
    pub fn in_logical_extent(&self, cell: GridCoord) -> bool {
        cell.x >= self.logical_extent_lo.x
            && cell.y >= self.logical_extent_lo.y
            && cell.x <= self.logical_extent_hi.x
            && cell.y <= self.logical_extent_hi.y
    }

    pub fn logical_extent(&self) -> (ICoord2D, ICoord2D) {
        (self.logical_extent_lo, self.logical_extent_hi)
    }

    pub fn set_logical_extent(&mut self, lo: ICoord2D, hi: ICoord2D) {
        self.logical_extent_lo = lo;
        self.logical_extent_hi = hi;
    }

    pub fn process_queue(&mut self, max_per_frame: usize) -> usize {
        // Terrain/queue drain is valid with zero objects (C++ pathfinder on empty maps).
        // Fail-closed object lookups happen per queued ObjectID below.

        // C++: if (!m_isMapReady) return;
        if !self.is_map_ready {
            return 0;
        }
        // C++: if needToCalculateZones → calculateZones and return (no queue drain).
        let dirty = self.zones.lock().map(|z| z.zones_dirty).unwrap_or(false);
        if dirty {
            self.recalculate_zones_from_cells();
            return 0;
        }

        // C++ processPathfindQueue: refresh m_logicalExtent from terrain extent.
        self.refresh_logical_extent();
        self.cumulative_cells_allocated.store(0, Ordering::Relaxed);

        // C++ while (m_cumulativeCellsAllocated < PATHFIND_CELLS_PER_FRAME && queue nonempty)
        let cell_budget = max_per_frame.max(1).min(PATHFIND_CELLS_PER_FRAME);
        let mut processed = 0;

        // Drain ObjectID ring (C++ primary path → ai->doPathfind).
        if let Ok(mut oq) = self.object_path_queue.lock() {
            while (self.cumulative_cells_allocated() as usize) < cell_budget && !oq.is_empty() {
                let Some(id) = oq.pop_front() else {
                    break;
                };
                drop(oq);
                // C++: Object* obj = findObjectByID; if (ai) ai->doPathfind(this);
                if id != INVALID_ID {
                    if let Some(ai) = OBJECT_REGISTRY
                        .with_object(id, |obj_g| obj_g.get_ai_update_interface())
                        .flatten()
                    {
                        if let Ok(mut ai_g) = ai.lock() {
                            ai_g.do_pathfind();
                        }
                    } else if let Ok(mut queue) = self.request_queue.lock() {
                        // Fallback: PathRequest residual for host/tests without registry object.
                        if let Some(pos) = queue.iter().position(|r| r.object_id == id) {
                            let req = queue.remove(pos).expect("pos");
                            drop(queue);
                            let _ = self.find_path_internal(req);
                        }
                    }
                }
                processed += 1;
                oq = self.object_path_queue.lock().unwrap();
            }
        }

        // Also drain PathRequest queue for host/tests without ObjectID ring.
        if let Ok(mut queue) = self.request_queue.lock() {
            while (self.cumulative_cells_allocated() as usize) < cell_budget && !queue.is_empty() {
                if let Some(request) = queue.pop_front() {
                    drop(queue);
                    let _ = self.find_path_internal(request);
                    processed += 1;
                    queue = self.request_queue.lock().unwrap();
                } else {
                    break;
                }
            }
        }

        processed
    }
}
