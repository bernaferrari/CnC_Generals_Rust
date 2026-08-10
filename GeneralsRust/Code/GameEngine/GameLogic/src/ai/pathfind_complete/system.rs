use super::*;

/// Complete pathfinding system
/// Matches C++ Pathfinder class at AIPathfind.h:568-846
pub struct PathfindingSystem {
    /// Core A* pathfinder
    pub(crate) pathfinder: Arc<Mutex<AStarPathfinder>>,

    /// Path optimizer
    pub(crate) optimizer: PathOptimizer,

    /// Bridge layers for elevated pathfinding
    /// Matches C++ m_layers at AIPathfind.h:832
    pub(crate) bridges: Vec<BridgeLayer>,

    /// Pathfind request queue (full PathRequest residual for tests/host).
    /// Matches C++ m_queuedPathfindRequests at AIPathfind.h:842
    pub(crate) request_queue: Arc<Mutex<VecDeque<PathRequest>>>,
    /// C++ m_queuedPathfindRequests ObjectID ring + head/tail.
    pub(crate) object_path_queue: Arc<Mutex<ObjectPathQueue>>,
    /// Goal cell tracking (ground/top + aircraft goals).
    pub(crate) goal_cells: Arc<Mutex<Vec<Vec<GoalCell>>>>,

    /// Cached paths
    pub(crate) path_cache: Arc<
        Mutex<
            HashMap<
                (
                    GridCoord,
                    GridCoord,
                    LocomotorSurfaceTypeMask,
                    bool,
                    bool,
                    u32,
                    bool,
                    ObjectID,
                    bool, // is_human
                ),
                PathResult,
            >,
        >,
    >,

    pub(crate) zones: Arc<Mutex<ZoneManager>>,

    /// Map dimensions
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// C++ m_isMapReady
    pub(crate) is_map_ready: bool,
    /// C++ AIUpdateInterface pathfind goal/cur cells per unit.
    pub(crate) unit_goal_cells: Arc<Mutex<HashMap<ObjectID, ICoord2D>>>,
    pub(crate) unit_pos_cells: Arc<Mutex<HashMap<ObjectID, ICoord2D>>>,
    /// C++ m_wallPieces / m_numWallPieces.
    pub(crate) wall_pieces: Vec<ObjectID>,
    /// Cells classified as walkable wall (LAYER_WALL clear).
    pub(crate) wall_cells: Arc<Mutex<HashSet<(i32, i32)>>>,
    /// C++ m_isTunneling
    pub(crate) is_tunneling: bool,
    /// C++ m_ignoreObstacleID
    pub(crate) ignore_obstacle_id: ObjectID,
    /// C++ m_wallHeight
    pub(crate) wall_height: f32,
    /// C++ m_cumulativeCellsAllocated
    pub(crate) cumulative_cells_allocated: AtomicI32,
    /// C++ m_moveAlliesDepth (LatchRestore recursion guard)
    pub(crate) move_allies_depth: i32,
    /// Residual open/closed cell counts for cleanOpenAndClosedLists.
    pub(crate) open_list_count: i32,
    pub(crate) closed_list_count: i32,
    /// C++ m_extent lo/hi as i32 pairs for CRC.
    pub(crate) extent_lo: ICoord2D,
    pub(crate) extent_hi: ICoord2D,
    /// C++ m_logicalExtent — playable terrain bounds in cells (human path clamp).
    pub(crate) logical_extent_lo: ICoord2D,
    pub(crate) logical_extent_hi: ICoord2D,
    /// C++ debugPath / debugPathPos (AI debug residual).
    pub(crate) debug_path: Option<PathResult>,
    pub(crate) debug_path_pos: Coord3D,
}

impl std::fmt::Debug for PathfindingSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathfindingSystem")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bridge_count", &self.bridges.len())
            .finish()
    }
}
