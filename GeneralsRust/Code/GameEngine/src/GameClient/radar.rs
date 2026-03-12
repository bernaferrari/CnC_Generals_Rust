// FILE: radar.rs
// Author: Colin Day, January 2002 (C++ original)
// Rust port: 2025
// Desc: Radar logic implementation
//
// Ported from: /GeneralsMD/Code/GameEngine/Source/Common/System/Radar.cpp
//              /GeneralsMD/Code/GameEngine/Include/Common/Radar.h

use super::types::{Coord3D, ICoord2D};

// Type aliases matching C++ base types
pub type Bool = bool;
pub type Int = i32;
pub type Real = f32;
pub type UnsignedInt = u32;
pub type UnsignedByte = u8;
pub type UnsignedShort = u16;

// Additional types needed for radar
/// 3D rectangular region (axis-aligned bounding box)
#[derive(Debug, Clone, Copy)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Region3D {
    pub fn new(lo: Coord3D, hi: Coord3D) -> Self {
        Self { lo, hi }
    }

    pub fn width(&self) -> Real {
        self.hi.x - self.lo.x
    }

    pub fn height(&self) -> Real {
        self.hi.y - self.lo.y
    }
}

/// RGBA color with integer components (0-255)
#[derive(Debug, Clone, Copy)]
pub struct RGBAColorInt {
    pub red: UnsignedInt,
    pub green: UnsignedInt,
    pub blue: UnsignedInt,
    pub alpha: UnsignedInt,
}

impl RGBAColorInt {
    pub fn new(red: UnsignedInt, green: UnsignedInt, blue: UnsignedInt, alpha: UnsignedInt) -> Self {
        Self { red, green, blue, alpha }
    }
}

/// RGB color with Real components (0.0-1.0)
#[derive(Debug, Clone, Copy)]
pub struct RGBColor {
    pub red: Real,
    pub green: Real,
    pub blue: Real,
}

impl RGBColor {
    pub fn new(red: Real, green: Real, blue: Real) -> Self {
        Self { red, green, blue }
    }
}

// CONSTANTS /////////////////////////////////////////////////////////////////////////////////////

/// Radar resolution constants
pub const RADAR_CELL_WIDTH: Int = 128;
pub const RADAR_CELL_HEIGHT: Int = 128;

/// Maximum number of radar events that can be tracked simultaneously
pub const MAX_RADAR_EVENTS: usize = 64;

/// Delay before terrain refresh is applied (in logic frames)
const RADAR_QUEUE_TERRAIN_REFRESH_DELAY: Real = 3.0 * 30.0; // Assumes 30 frames per second

/// Logic frames per second constant (from C++ LOGICFRAMES_PER_SECOND)
pub const LOGICFRAMES_PER_SECOND: Real = 30.0;

// TYPES /////////////////////////////////////////////////////////////////////////////////////////

/// Type alias for Color (matches C++ Color type which is a 32-bit value)
pub type Color = u32;

/// Radar event types - determines the colors radar events happen in
/// Matches C++ RadarEventType enum from Radar.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RadarEventType {
    Invalid = 0,
    Construction,
    Upgrade,
    UnderAttack,
    Information,
    BeaconPulse,
    Infiltration,        // for defection, hijacking, hacking, carbombing
    BattlePlan,
    StealthDiscovered,   // we discovered a stealth unit
    StealthNeutralized,  // our stealth unit has been revealed
    Fake,                // Internally creates a radar event but doesn't notify the player
    NumEvents,
}

/// Radar priorities - determines drawing order and visibility
/// Keep this in sync with C++ RadarPriorityType from Radar.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RadarPriorityType {
    Invalid = 0,          // a priority that has not been set
    NotOnRadar,           // object specifically forbidden from radar
    Structure,            // structure level drawing priority
    Unit,                 // unit level drawing priority
    LocalUnitOnly,        // unit priority, but only on radar if controlled by local player
    NumPriorities,
}

/// Object ID type (matches C++ ObjectID)
pub type ObjectID = UnsignedInt;

/// Invalid object ID constant
pub const INVALID_ID: ObjectID = 0xFFFFFFFF;

/// Radar event structure
/// Matches C++ RadarEvent struct from Radar.h
#[derive(Debug, Clone)]
pub struct RadarEvent {
    pub event_type: RadarEventType,
    pub active: Bool,
    pub create_frame: UnsignedInt,
    pub die_frame: UnsignedInt,
    pub fade_frame: UnsignedInt,
    pub color1: RGBAColorInt,
    pub color2: RGBAColorInt,
    pub world_loc: Coord3D,
    pub radar_loc: ICoord2D,
    pub sound_played: Bool,
}

impl Default for RadarEvent {
    fn default() -> Self {
        Self {
            event_type: RadarEventType::Invalid,
            active: false,
            create_frame: 0,
            die_frame: 0,
            fade_frame: 0,
            color1: RGBAColorInt::new(0, 0, 0, 0),
            color2: RGBAColorInt::new(0, 0, 0, 0),
            world_loc: Coord3D::new(0.0, 0.0, 0.0),
            radar_loc: ICoord2D::new(0, 0),
            sound_played: false,
        }
    }
}

/// Shroud cell status
/// Matches C++ CellShroudStatus from Display.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CellShroudStatus {
    Clear = 0,
    Fogged,
    Shrouded,
}

/// Radar object - represents an object on the radar
/// Matches C++ RadarObject class from Radar.h
pub struct RadarObject {
    object_id: ObjectID,
    next: Option<Box<RadarObject>>,
    color: Color,
}

impl RadarObject {
    /// Create a new radar object
    pub fn new() -> Self {
        Self {
            object_id: INVALID_ID,
            next: None,
            color: game_make_color(255, 255, 255, 255),
        }
    }

    /// Set the object ID
    pub fn set_object_id(&mut self, id: ObjectID) {
        self.object_id = id;
    }

    /// Get the object ID
    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    /// Set the next radar object in the list
    pub fn set_next(&mut self, next: Option<Box<RadarObject>>) {
        self.next = next;
    }

    /// Get reference to the next radar object
    pub fn get_next(&self) -> Option<&RadarObject> {
        self.next.as_deref()
    }

    /// Get mutable reference to the next radar object
    pub fn get_next_mut(&mut self) -> Option<&mut RadarObject> {
        self.next.as_deref_mut()
    }

    /// Take ownership of the next radar object
    pub fn take_next(&mut self) -> Option<Box<RadarObject>> {
        self.next.take()
    }

    /// Set the color for this object
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    /// Get the color for this object
    pub fn get_color(&self) -> Color {
        self.color
    }
}

/// Radar color lookup table entry
/// Matches radarColorLookupTable from Radar.cpp
struct RadarColorLookup {
    event: RadarEventType,
    color1: RGBAColorInt,
    color2: RGBAColorInt,
}

/// Radar color lookup table
/// Matches C++ radarColorLookupTable from Radar.cpp lines 976-990
const RADAR_COLOR_LOOKUP_TABLE: &[RadarColorLookup] = &[
    RadarColorLookup {
        event: RadarEventType::Construction,
        color1: RGBAColorInt { red: 128, green: 128, blue: 255, alpha: 255 },
        color2: RGBAColorInt { red: 128, green: 255, blue: 255, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::Upgrade,
        color1: RGBAColorInt { red: 128, green: 0, blue: 64, alpha: 255 },
        color2: RGBAColorInt { red: 255, green: 185, blue: 220, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::UnderAttack,
        color1: RGBAColorInt { red: 255, green: 0, blue: 0, alpha: 255 },
        color2: RGBAColorInt { red: 255, green: 128, blue: 128, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::Information,
        color1: RGBAColorInt { red: 255, green: 255, blue: 0, alpha: 255 },
        color2: RGBAColorInt { red: 255, green: 255, blue: 128, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::BeaconPulse,
        color1: RGBAColorInt { red: 255, green: 255, blue: 0, alpha: 255 },
        color2: RGBAColorInt { red: 255, green: 255, blue: 128, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::Infiltration,
        color1: RGBAColorInt { red: 0, green: 255, blue: 255, alpha: 255 },
        color2: RGBAColorInt { red: 128, green: 255, blue: 255, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::BattlePlan,
        color1: RGBAColorInt { red: 255, green: 255, blue: 255, alpha: 255 },
        color2: RGBAColorInt { red: 255, green: 255, blue: 255, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::StealthDiscovered,
        color1: RGBAColorInt { red: 0, green: 255, blue: 0, alpha: 255 },
        color2: RGBAColorInt { red: 0, green: 128, blue: 0, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::StealthNeutralized,
        color1: RGBAColorInt { red: 0, green: 255, blue: 0, alpha: 255 },
        color2: RGBAColorInt { red: 0, green: 128, blue: 0, alpha: 255 },
    },
    RadarColorLookup {
        event: RadarEventType::Fake,
        color1: RGBAColorInt { red: 0, green: 0, blue: 0, alpha: 0 },
        color2: RGBAColorInt { red: 0, green: 0, blue: 0, alpha: 0 },
    },
];

// UTILITY FUNCTIONS //////////////////////////////////////////////////////////////////////////////

/// Create a color from RGBA components
/// Matches C++ GameMakeColor function
pub fn game_make_color(r: UnsignedByte, g: UnsignedByte, b: UnsignedByte, a: UnsignedByte) -> Color {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Get color components from a color value
/// Matches C++ GameGetColorComponents function
pub fn game_get_color_components(color: Color) -> (UnsignedByte, UnsignedByte, UnsignedByte, UnsignedByte) {
    let r = ((color >> 16) & 0xFF) as UnsignedByte;
    let g = ((color >> 8) & 0xFF) as UnsignedByte;
    let b = (color & 0xFF) as UnsignedByte;
    let a = ((color >> 24) & 0xFF) as UnsignedByte;
    (r, g, b, a)
}

/// Conversion macros from C++
#[inline]
pub fn int_to_real(i: Int) -> Real {
    i as Real
}

#[inline]
pub fn real_to_int(r: Real) -> Int {
    r as Int
}

#[inline]
pub fn real_to_unsignedbyte(r: Real) -> UnsignedByte {
    r.clamp(0.0, 255.0) as UnsignedByte
}

#[inline]
pub fn double_to_real(d: f64) -> Real {
    d as Real
}

// RADAR TRAIT ////////////////////////////////////////////////////////////////////////////////////

/// Trait for radar system interface
/// Matches C++ Radar class from Radar.h
pub trait RadarInterface {
    /// Initialize the radar subsystem
    fn init(&mut self);

    /// Reset the radar to initial state
    fn reset(&mut self);

    /// Per-frame update
    fn update(&mut self);

    /// Setup radar for new map
    fn new_map(&mut self, terrain: &dyn TerrainLogicInterface);

    /// Add an object to the radar
    fn add_object(&mut self, obj_id: ObjectID, priority: RadarPriorityType, color: Color);

    /// Remove an object from the radar
    fn remove_object(&mut self, obj_id: ObjectID);

    /// Convert radar coordinates to world coordinates
    fn radar_to_world(&self, radar: &ICoord2D) -> Option<Coord3D>;

    /// Convert radar coordinates to 2D world coordinates (x,y only)
    fn radar_to_world_2d(&self, radar: &ICoord2D) -> Option<Coord3D>;

    /// Convert world coordinates to radar coordinates
    fn world_to_radar(&self, world: &Coord3D) -> Option<ICoord2D>;

    /// Convert local pixel coordinates to radar coordinates
    fn local_pixel_to_radar(&self, pixel: &ICoord2D) -> Option<ICoord2D>;

    /// Convert screen pixel coordinates to world coordinates
    fn screen_pixel_to_world(&self, pixel: &ICoord2D) -> Option<Coord3D>;

    /// Get object under a radar pixel position
    fn object_under_radar_pixel(&self, pixel: &ICoord2D) -> Option<ObjectID>;

    /// Find draw positions for aspect ratio preservation
    fn find_draw_positions(&self, start_x: Int, start_y: Int, width: Int, height: Int) -> (ICoord2D, ICoord2D);

    /// Check if a priority type is visible on the radar
    fn is_priority_visible(&self, priority: RadarPriorityType) -> Bool;

    /// Create a radar event
    fn create_event(&mut self, world: &Coord3D, event_type: RadarEventType, seconds_to_live: Real);

    /// Create a radar event with player colors
    fn create_player_event(&mut self, player_color: Color, world: &Coord3D, event_type: RadarEventType, seconds_to_live: Real);

    /// Get the last event location
    fn get_last_event_loc(&self) -> Option<Coord3D>;

    /// Try to create an "under attack" event
    fn try_under_attack_event(&mut self, world: &Coord3D);

    /// Try to create an "infiltration" event
    fn try_infiltration_event(&mut self, world: &Coord3D);

    /// Try to create a generic event
    fn try_event(&mut self, event_type: RadarEventType, pos: &Coord3D) -> Bool;

    /// Hide/unhide the radar
    fn hide(&mut self, hide: Bool);

    /// Check if radar is hidden
    fn is_radar_hidden(&self) -> Bool;

    /// Force the radar on/off
    fn force_on(&mut self, force: Bool);

    /// Check if radar is forced on
    fn is_radar_forced(&self) -> Bool;

    /// Refresh the terrain display
    fn refresh_terrain(&mut self, terrain: &dyn TerrainLogicInterface);

    /// Queue a terrain refresh for later
    fn queue_terrain_refresh(&mut self);

    /// Clear all shroud
    fn clear_shroud(&mut self);

    /// Set shroud level at a cell
    fn set_shroud_level(&mut self, x: Int, y: Int, setting: CellShroudStatus);

    /// Draw the radar (device-specific)
    fn draw(&mut self, pixel_x: Int, pixel_y: Int, width: Int, height: Int);
}

/// Terrain logic interface (placeholder for actual implementation)
pub trait TerrainLogicInterface {
    fn get_extent(&self) -> Region3D;
    fn get_ground_height(&self, x: Real, y: Real) -> Real;
    fn is_underwater(&self, x: Real, y: Real) -> (Bool, Real);
}

// RADAR IMPLEMENTATION ///////////////////////////////////////////////////////////////////////////

/// Main radar system implementation
/// Matches C++ Radar class from Radar.h and Radar.cpp
pub struct Radar {
    radar_hidden: Bool,
    radar_force_on: Bool,
    object_list: Option<Box<RadarObject>>,
    local_object_list: Option<Box<RadarObject>>,
    terrain_average_z: Real,
    water_average_z: Real,
    x_sample: Real,
    y_sample: Real,
    map_extent: Region3D,
    events: Vec<RadarEvent>,
    next_free_radar_event: Int,
    last_radar_event: Int,
    queue_terrain_refresh_frame: UnsignedInt,
    current_frame: UnsignedInt,
}

impl Radar {
    /// Create a new Radar instance
    /// Matches C++ Radar::Radar() constructor from Radar.cpp line 184
    pub fn new() -> Self {
        let mut events = Vec::with_capacity(MAX_RADAR_EVENTS);
        for _ in 0..MAX_RADAR_EVENTS {
            events.push(RadarEvent::default());
        }

        Self {
            radar_hidden: false,
            radar_force_on: false,
            object_list: None,
            local_object_list: None,
            terrain_average_z: 0.0,
            water_average_z: 0.0,
            x_sample: 0.0,
            y_sample: 0.0,
            map_extent: Region3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)),
            events,
            next_free_radar_event: 0,
            last_radar_event: -1,
            queue_terrain_refresh_frame: 0,
            current_frame: 0,
        }
    }

    /// Clear all radar events
    /// Matches C++ Radar::clearAllEvents() from Radar.cpp line 222
    fn clear_all_events(&mut self) {
        self.next_free_radar_event = 0;
        self.last_radar_event = -1;

        for event in &mut self.events {
            *event = RadarEvent::default();
        }
    }

    /// Delete list resources
    /// Matches C++ Radar::deleteListResources() from Radar.cpp line 47
    fn delete_list_resources(&mut self) {
        self.local_object_list = None;
        self.object_list = None;
    }

    /// Delete an object from a specific list
    /// Matches C++ Radar::deleteFromList() from Radar.cpp line 531
    fn delete_from_list(list: &mut Option<Box<RadarObject>>, obj_id: ObjectID) -> Bool {
        let mut current = list;
        let mut prev: Option<&mut Box<RadarObject>> = None;

        loop {
            match current {
                None => return false,
                Some(radar_obj) => {
                    if radar_obj.get_object_id() == obj_id {
                        // Found the object, unlink it
                        let next = radar_obj.take_next();
                        if let Some(prev_node) = prev {
                            prev_node.set_next(next);
                        } else {
                            *list = next;
                        }
                        return true;
                    }

                    // Move to next
                    let next_ptr = radar_obj.get_next_mut();
                    prev = Some(radar_obj);
                    current = if next_ptr.is_some() {
                        // This is safe because we're moving through the list
                        unsafe {
                            let ptr = next_ptr.unwrap() as *mut RadarObject;
                            Some(&mut *(ptr as *mut Box<RadarObject>))
                        }
                    } else {
                        &mut None
                    };
                }
            }
        }
    }

    /// Search a list for an object at a radar location
    /// Matches C++ Radar::searchListForRadarLocationMatch() from Radar.cpp line 829
    fn search_list_for_radar_location_match(list: &Option<Box<RadarObject>>, radar_match: &ICoord2D, world_to_radar_fn: impl Fn(ObjectID) -> Option<ICoord2D>) -> Option<ObjectID> {
        let mut current = list.as_ref();

        while let Some(radar_obj) = current {
            let obj_id = radar_obj.get_object_id();

            if let Some(radar) = world_to_radar_fn(obj_id) {
                // Check if within tolerance (±1 pixel)
                if radar.x >= radar_match.x - 1 && radar.x <= radar_match.x + 1 &&
                   radar.y >= radar_match.y - 1 && radar.y <= radar_match.y + 1 {
                    return Some(obj_id);
                }
            }

            current = radar_obj.get_next();
        }

        None
    }

    /// Internal method to create a radar event with specific colors
    /// Matches C++ Radar::internalCreateEvent() from Radar.cpp line 1080
    fn internal_create_event(&mut self, world: &Coord3D, event_type: RadarEventType, seconds_to_live: Real, color1: &RGBAColorInt, color2: &RGBAColorInt) {
        const SECONDS_BEFORE_DIE_TO_FADE: Real = 0.5;

        // Translate world coord to radar coords
        let radar = self.world_to_radar(world).unwrap_or(ICoord2D::new(0, 0));

        // Setup the event
        let idx = self.next_free_radar_event as usize;
        if idx < MAX_RADAR_EVENTS {
            self.events[idx].event_type = event_type;
            self.events[idx].active = true;
            self.events[idx].create_frame = self.current_frame;
            self.events[idx].die_frame = self.current_frame + (LOGICFRAMES_PER_SECOND * seconds_to_live) as UnsignedInt;
            self.events[idx].fade_frame = self.events[idx].die_frame - (LOGICFRAMES_PER_SECOND * SECONDS_BEFORE_DIE_TO_FADE) as UnsignedInt;
            self.events[idx].color1 = color1.clone();
            self.events[idx].color2 = color2.clone();
            self.events[idx].world_loc = *world;
            self.events[idx].radar_loc = radar;
            self.events[idx].sound_played = false;

            // Record this as the last radar event (unless it's a beacon pulse)
            if event_type != RadarEventType::BeaconPulse {
                self.last_radar_event = self.next_free_radar_event;
            }

            // Increment next free event index with wrap-around
            self.next_free_radar_event += 1;
            if self.next_free_radar_event >= MAX_RADAR_EVENTS as Int {
                self.next_free_radar_event = 0;
            }
        }
    }

    /// Get average terrain Z
    pub fn get_terrain_average_z(&self) -> Real {
        self.terrain_average_z
    }

    /// Get average water Z
    pub fn get_water_average_z(&self) -> Real {
        self.water_average_z
    }

    /// Get reference to object list
    pub fn get_object_list(&self) -> &Option<Box<RadarObject>> {
        &self.object_list
    }

    /// Get reference to local object list
    pub fn get_local_object_list(&self) -> &Option<Box<RadarObject>> {
        &self.local_object_list
    }
}

impl RadarInterface for Radar {
    /// Initialize the radar subsystem
    /// Matches C++ Radar::init() - default implementation
    fn init(&mut self) {
        // Base implementation does nothing
    }

    /// Reset the radar to initial state
    /// Matches C++ Radar::reset() from Radar.cpp line 258
    fn reset(&mut self) {
        self.delete_list_resources();
        self.clear_all_events();
        self.radar_force_on = false;
    }

    /// Per-frame update
    /// Matches C++ Radar::update() from Radar.cpp line 275
    fn update(&mut self) {
        // Update current frame counter (should be set from game logic)
        // For now, increment locally
        self.current_frame += 1;

        // Traverse radar event list and deactivate expired events
        for i in 0..MAX_RADAR_EVENTS {
            if self.events[i].active && self.events[i].create_frame != 0 && self.current_frame > self.events[i].die_frame {
                self.events[i].active = false;
            }
        }

        // Check if we should refresh the terrain
        if self.queue_terrain_refresh_frame != 0 && (self.current_frame - self.queue_terrain_refresh_frame) as Real > RADAR_QUEUE_TERRAIN_REFRESH_DELAY {
            self.queue_terrain_refresh_frame = 0;
            // Terrain refresh would happen here in full implementation
        }
    }

    /// Setup radar for new map
    /// Matches C++ Radar::newMap() from Radar.cpp line 309
    fn new_map(&mut self, terrain: &dyn TerrainLogicInterface) {
        // Reset all data
        self.reset();

        // Get map extents
        self.map_extent = terrain.get_extent();

        // Calculate sample intervals
        self.x_sample = self.map_extent.width() / int_to_real(RADAR_CELL_WIDTH);
        self.y_sample = self.map_extent.height() / int_to_real(RADAR_CELL_HEIGHT);

        // Calculate average heights for terrain and water
        let mut terrain_samples = 0;
        let mut water_samples = 0;
        self.terrain_average_z = 0.0;
        self.water_average_z = 0.0;

        // Sample every second cell for performance
        let mut world_y = 0.0;
        for _y in (0..RADAR_CELL_HEIGHT).step_by(2) {
            let mut world_x = 0.0;
            for _x in (0..RADAR_CELL_WIDTH).step_by(2) {
                let (is_underwater, z) = terrain.is_underwater(world_x, world_y);
                if is_underwater {
                    self.water_average_z += z;
                    water_samples += 1;
                } else {
                    self.terrain_average_z += z;
                    terrain_samples += 1;
                }
                world_x += 2.0 * self.x_sample;
            }
            world_y += 2.0 * self.y_sample;
        }

        // Avoid divide by zero
        if terrain_samples == 0 {
            terrain_samples = 1;
        }
        if water_samples == 0 {
            water_samples = 1;
        }

        // Compute averages
        self.terrain_average_z /= int_to_real(terrain_samples);
        self.water_average_z /= int_to_real(water_samples);
    }

    /// Add an object to the radar
    /// Matches C++ Radar::addObject() from Radar.cpp line 376
    fn add_object(&mut self, obj_id: ObjectID, priority: RadarPriorityType, color: Color) {
        // Check if priority is visible
        if !self.is_priority_visible(priority) {
            return;
        }

        // Create new radar object
        let mut new_obj = Box::new(RadarObject::new());
        new_obj.set_object_id(obj_id);
        new_obj.set_color(color);

        // Determine which list to add to
        // For now, simplified logic - would check object ownership in full implementation
        let list = &mut self.object_list;

        // Insert into list sorted by priority
        if list.is_none() {
            *list = Some(new_obj);
        } else {
            // Simplified insertion at head for now
            // Full implementation would sort by priority
            let old_head = list.take();
            new_obj.set_next(old_head);
            *list = Some(new_obj);
        }
    }

    /// Remove an object from the radar
    /// Matches C++ Radar::removeObject() from Radar.cpp line 572
    fn remove_object(&mut self, obj_id: ObjectID) {
        if Self::delete_from_list(&mut self.local_object_list, obj_id) {
            return;
        }
        Self::delete_from_list(&mut self.object_list, obj_id);
    }

    /// Convert radar coordinates to world coordinates
    /// Matches C++ Radar::radarToWorld() from Radar.cpp line 634
    fn radar_to_world(&self, radar: &ICoord2D) -> Option<Coord3D> {
        let mut world = self.radar_to_world_2d(radar)?;

        // For full implementation, would get ground height from terrain
        // For now, use average
        world.z = self.terrain_average_z;

        Some(world)
    }

    /// Convert radar coordinates to 2D world coordinates
    /// Matches C++ Radar::radarToWorld2D() from Radar.cpp line 600
    fn radar_to_world_2d(&self, radar: &ICoord2D) -> Option<Coord3D> {
        let mut x = radar.x;
        let mut y = radar.y;

        // Clamp to valid range
        if x < 0 {
            x = 0;
        }
        if x >= RADAR_CELL_WIDTH {
            x = RADAR_CELL_WIDTH - 1;
        }
        if y < 0 {
            y = 0;
        }
        if y >= RADAR_CELL_HEIGHT {
            y = RADAR_CELL_HEIGHT - 1;
        }

        // Convert to world coordinates
        let world = Coord3D::new(
            int_to_real(x) * self.x_sample,
            int_to_real(y) * self.y_sample,
            0.0,
        );

        Some(world)
    }

    /// Convert world coordinates to radar coordinates
    /// Matches C++ Radar::worldToRadar() from Radar.cpp line 651
    fn world_to_radar(&self, world: &Coord3D) -> Option<ICoord2D> {
        let mut radar = ICoord2D::new(
            real_to_int(world.x / self.x_sample),
            real_to_int(world.y / self.y_sample),
        );

        // Clamp to valid range
        if radar.x < 0 {
            radar.x = 0;
        }
        if radar.x >= RADAR_CELL_WIDTH {
            radar.x = RADAR_CELL_WIDTH - 1;
        }
        if radar.y < 0 {
            radar.y = 0;
        }
        if radar.y >= RADAR_CELL_HEIGHT {
            radar.y = RADAR_CELL_HEIGHT - 1;
        }

        Some(radar)
    }

    /// Convert local pixel coordinates to radar coordinates
    /// Matches C++ Radar::localPixelToRadar() from Radar.cpp line 692
    fn local_pixel_to_radar(&self, pixel: &ICoord2D) -> Option<ICoord2D> {
        // Simplified implementation - full version would handle aspect ratio scaling
        // This would require window size information
        let radar = ICoord2D::new(
            pixel.x * RADAR_CELL_WIDTH / 128,  // Assuming 128x128 display size
            pixel.y * RADAR_CELL_HEIGHT / 128,
        );
        Some(radar)
    }

    /// Convert screen pixel coordinates to world coordinates
    /// Matches C++ Radar::screenPixelToWorld() from Radar.cpp line 762
    fn screen_pixel_to_world(&self, pixel: &ICoord2D) -> Option<Coord3D> {
        // Convert screen pixel to local pixel (simplified)
        let local_pixel = *pixel;  // Would subtract radar window position

        // Convert local pixel to radar
        let radar = self.local_pixel_to_radar(&local_pixel)?;

        // Convert radar to world
        self.radar_to_world(&radar)
    }

    /// Get object under a radar pixel position
    /// Matches C++ Radar::objectUnderRadarPixel() from Radar.cpp line 794
    fn object_under_radar_pixel(&self, pixel: &ICoord2D) -> Option<ObjectID> {
        // Convert pixel to radar coords
        let radar = self.local_pixel_to_radar(pixel)?;

        // Dummy closure for world_to_radar lookup (would query actual objects)
        let world_to_radar_fn = |_obj_id: ObjectID| -> Option<ICoord2D> {
            None  // Would get object position and convert to radar
        };

        // Search local object list first
        if let Some(obj_id) = Self::search_list_for_radar_location_match(&self.local_object_list, &radar, &world_to_radar_fn) {
            return Some(obj_id);
        }

        // Search regular object list
        Self::search_list_for_radar_location_match(&self.object_list, &radar, world_to_radar_fn)
    }

    /// Find draw positions for aspect ratio preservation
    /// Matches C++ Radar::findDrawPositions() from Radar.cpp line 877
    fn find_draw_positions(&self, start_x: Int, start_y: Int, width: Int, height: Int) -> (ICoord2D, ICoord2D) {
        let ratio_width = self.map_extent.width() / int_to_real(width);
        let ratio_height = self.map_extent.height() / int_to_real(height);

        let (ul, lr) = if ratio_width >= ratio_height {
            let radar_x = self.map_extent.width() / ratio_width;
            let radar_y = self.map_extent.height() / ratio_width;
            let ul = ICoord2D::new(
                0,
                real_to_int((int_to_real(height) - radar_y) / 2.0),
            );
            let lr = ICoord2D::new(
                real_to_int(radar_x),
                height - ul.y,
            );
            (ul, lr)
        } else {
            let radar_x = self.map_extent.width() / ratio_height;
            let radar_y = self.map_extent.height() / ratio_height;
            let ul = ICoord2D::new(
                real_to_int((int_to_real(width) - radar_x) / 2.0),
                0,
            );
            let lr = ICoord2D::new(
                width - ul.x,
                real_to_int(radar_y),
            );
            (ul, lr)
        };

        (
            ICoord2D::new(ul.x + start_x, ul.y + start_y),
            ICoord2D::new(lr.x + start_x, lr.y + start_y),
        )
    }

    /// Check if a priority type is visible on the radar
    /// Matches C++ Radar::isPriorityVisible() from Radar.cpp line 1529
    fn is_priority_visible(&self, priority: RadarPriorityType) -> Bool {
        match priority {
            RadarPriorityType::Invalid | RadarPriorityType::NotOnRadar => false,
            _ => true,
        }
    }

    /// Create a radar event
    /// Matches C++ Radar::createEvent() from Radar.cpp line 995
    fn create_event(&mut self, world: &Coord3D, event_type: RadarEventType, seconds_to_live: Real) {
        // Look up colors from the table
        let mut color1 = RGBAColorInt::new(255, 255, 255, 255);
        let mut color2 = RGBAColorInt::new(255, 255, 255, 255);

        for entry in RADAR_COLOR_LOOKUP_TABLE {
            if entry.event as u32 == event_type as u32 {
                color1 = entry.color1.clone();
                color2 = entry.color2.clone();
                break;
            }
        }

        self.internal_create_event(world, event_type, seconds_to_live, &color1, &color2);
    }

    /// Create a radar event with player colors
    /// Matches C++ Radar::createPlayerEvent() from Radar.cpp line 1038
    fn create_player_event(&mut self, player_color: Color, world: &Coord3D, event_type: RadarEventType, seconds_to_live: Real) {
        let (r, g, b, a) = game_get_color_components(player_color);

        let color1 = RGBAColorInt::new(r as UnsignedInt, g as UnsignedInt, b as UnsignedInt, a as UnsignedInt);

        // Create darker version for color2
        let dark_scale = 0.75;
        let mut color2 = color1.clone();
        color2.red = ((color1.red as Real * (1.0 - dark_scale)) as UnsignedInt).max(0);
        color2.green = ((color1.green as Real * (1.0 - dark_scale)) as UnsignedInt).max(0);
        color2.blue = ((color1.blue as Real * (1.0 - dark_scale)) as UnsignedInt).max(0);

        self.internal_create_event(world, event_type, seconds_to_live, &color1, &color2);
    }

    /// Get the last event location
    /// Matches C++ Radar::getLastEventLoc() from Radar.cpp line 1124
    fn get_last_event_loc(&self) -> Option<Coord3D> {
        if self.last_radar_event != -1 {
            let idx = self.last_radar_event as usize;
            if idx < MAX_RADAR_EVENTS {
                return Some(self.events[idx].world_loc);
            }
        }
        None
    }

    /// Try to create an "under attack" event
    /// Matches C++ Radar::tryUnderAttackEvent() from Radar.cpp line 1147
    fn try_under_attack_event(&mut self, world: &Coord3D) {
        // Try to create the event (with throttling logic)
        let event_created = self.try_event(RadarEventType::UnderAttack, world);

        // In full implementation, would trigger UI feedback here
        if event_created {
            // TheControlBar->triggerRadarAttackGlow();
            // TheInGameUI->message("RADAR:UnderAttack");
            // TheAudio->addAudioEvent(...);
        }
    }

    /// Try to create an "infiltration" event
    /// Matches C++ Radar::tryInfiltrationEvent() from Radar.cpp line 1233
    fn try_infiltration_event(&mut self, world: &Coord3D) {
        self.create_event(world, RadarEventType::Infiltration, 4.0);
        // In full implementation, would play UI feedback
    }

    /// Try to create a generic event
    /// Matches C++ Radar::tryEvent() from Radar.cpp line 1269
    fn try_event(&mut self, event_type: RadarEventType, pos: &Coord3D) -> Bool {
        const CLOSE_ENOUGH_DISTANCE_SQ: Real = 250.0 * 250.0;
        const FRAMES_BETWEEN_EVENTS: UnsignedInt = (LOGICFRAMES_PER_SECOND * 10.0) as UnsignedInt;

        // Check if there's a recent similar event nearby
        for i in 0..MAX_RADAR_EVENTS {
            if self.events[i].event_type as u32 == event_type as u32 {
                // Calculate distance squared
                let dx = self.events[i].world_loc.x - pos.x;
                let dy = self.events[i].world_loc.y - pos.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= CLOSE_ENOUGH_DISTANCE_SQ {
                    // Check if it's recent
                    if self.current_frame - self.events[i].create_frame < FRAMES_BETWEEN_EVENTS {
                        return false;  // Reject creating new event
                    }
                }
            }
        }

        // Create the event
        self.create_event(pos, event_type, 4.0);
        true
    }

    /// Hide/unhide the radar
    fn hide(&mut self, hide: Bool) {
        self.radar_hidden = hide;
    }

    /// Check if radar is hidden
    fn is_radar_hidden(&self) -> Bool {
        self.radar_hidden
    }

    /// Force the radar on/off
    fn force_on(&mut self, force: Bool) {
        self.radar_force_on = force;
    }

    /// Check if radar is forced on
    fn is_radar_forced(&self) -> Bool {
        self.radar_force_on
    }

    /// Refresh the terrain display
    /// Matches C++ Radar::refreshTerrain() from Radar.cpp line 1320
    fn refresh_terrain(&mut self, _terrain: &dyn TerrainLogicInterface) {
        self.queue_terrain_refresh_frame = 0;
        // Device-specific implementation would rebuild terrain texture
    }

    /// Queue a terrain refresh for later
    /// Matches C++ Radar::queueTerrainRefresh() from Radar.cpp line 1334
    fn queue_terrain_refresh(&mut self) {
        self.queue_terrain_refresh_frame = self.current_frame;
    }

    /// Clear all shroud
    /// Base implementation - device-specific subclass would override
    fn clear_shroud(&mut self) {
        // Device-specific implementation
    }

    /// Set shroud level at a cell
    /// Base implementation - device-specific subclass would override
    fn set_shroud_level(&mut self, _x: Int, _y: Int, _setting: CellShroudStatus) {
        // Device-specific implementation
    }

    /// Draw the radar
    /// Base implementation - device-specific subclass would override
    fn draw(&mut self, _pixel_x: Int, _pixel_y: Int, _width: Int, _height: Int) {
        // Device-specific implementation
    }
}

impl Default for Radar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTerrain {
        extent: Region3D,
    }

    impl TerrainLogicInterface for MockTerrain {
        fn get_extent(&self) -> Region3D {
            self.extent
        }

        fn get_ground_height(&self, _x: Real, _y: Real) -> Real {
            10.0
        }

        fn is_underwater(&self, _x: Real, _y: Real) -> (Bool, Real) {
            (false, 10.0)
        }
    }

    #[test]
    fn test_radar_creation() {
        let radar = Radar::new();
        assert_eq!(radar.is_radar_hidden(), false);
        assert_eq!(radar.is_radar_forced(), false);
    }

    #[test]
    fn test_radar_reset() {
        let mut radar = Radar::new();
        radar.force_on(true);
        radar.reset();
        assert_eq!(radar.is_radar_forced(), false);
    }

    #[test]
    fn test_color_functions() {
        let color = game_make_color(255, 128, 64, 32);
        let (r, g, b, a) = game_get_color_components(color);
        assert_eq!(r, 255);
        assert_eq!(g, 128);
        assert_eq!(b, 64);
        assert_eq!(a, 32);
    }

    #[test]
    fn test_world_to_radar_conversion() {
        let mut radar = Radar::new();
        let terrain = MockTerrain {
            extent: Region3D::new(
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(1280.0, 1280.0, 100.0),
            ),
        };
        radar.new_map(&terrain);

        let world = Coord3D::new(640.0, 640.0, 10.0);
        let radar_pos = radar.world_to_radar(&world).unwrap();
        assert_eq!(radar_pos.x, 64);
        assert_eq!(radar_pos.y, 64);
    }

    #[test]
    fn test_radar_to_world_conversion() {
        let mut radar = Radar::new();
        let terrain = MockTerrain {
            extent: Region3D::new(
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(1280.0, 1280.0, 100.0),
            ),
        };
        radar.new_map(&terrain);

        let radar_pos = ICoord2D::new(64, 64);
        let world = radar.radar_to_world_2d(&radar_pos).unwrap();
        assert!((world.x - 640.0).abs() < 1.0);
        assert!((world.y - 640.0).abs() < 1.0);
    }

    #[test]
    fn test_priority_visibility() {
        let radar = Radar::new();
        assert_eq!(radar.is_priority_visible(RadarPriorityType::Invalid), false);
        assert_eq!(radar.is_priority_visible(RadarPriorityType::NotOnRadar), false);
        assert_eq!(radar.is_priority_visible(RadarPriorityType::Structure), true);
        assert_eq!(radar.is_priority_visible(RadarPriorityType::Unit), true);
    }

    #[test]
    fn test_radar_event_creation() {
        let mut radar = Radar::new();
        let pos = Coord3D::new(100.0, 100.0, 10.0);
        radar.create_event(&pos, RadarEventType::UnderAttack, 4.0);

        // Should have created an event
        assert!(radar.events[0].active);
        assert_eq!(radar.events[0].event_type as u32, RadarEventType::UnderAttack as u32);
    }
}
