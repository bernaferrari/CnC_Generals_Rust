//! XY location tracker mirroring WWLib `trackxy.h`.
//!
//! This class is used to keep track of a "current XY location". The Surface class uses this, but
//! it can be used for any such purpose.
//!
//! # Examples
//!
//! ```rust
//! use wwlib::trackxy::TrackXY;
//!
//! let mut tracker = TrackXY::new();
//! tracker.set(100, 200);
//! assert_eq!(tracker.get_x(), 100);
//! assert_eq!(tracker.get_y(), 200);
//! ```

/// Tracks a current XY location.
///
/// It is often convenient to have a "current location" for a surface. The
/// use of this location is arbitrary and outside the scope of this class.
#[derive(Clone, Copy, Debug)]
pub struct TrackXY {
    /// Keeps track of the current location. The use of this
    /// current location is outside the scope of this class, but it can be quite
    /// useful for other support functions.
    x: i32,
    y: i32,
}

impl TrackXY {
    /// Create a new TrackXY with position (0, 0).
    ///
    /// Matches C++ default constructor `TrackXY(void) : X(0), Y(0) {}`.
    pub fn new() -> Self {
        TrackXY { x: 0, y: 0 }
    }

    /// Create a new TrackXY with the specified position.
    pub fn with_position(x: i32, y: i32) -> Self {
        TrackXY { x, y }
    }

    /// Set the current location.
    ///
    /// Matches C++ `void Set(int x, int y) {X = x; Y = y;}`.
    pub fn set(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Get the X coordinate.
    ///
    /// Matches C++ `int Get_X(void) const {return(X);}`.
    pub fn get_x(&self) -> i32 {
        self.x
    }

    /// Get the Y coordinate.
    ///
    /// Matches C++ `int Get_Y(void) const {return(Y);}`.
    pub fn get_y(&self) -> i32 {
        self.y
    }

    /// Get both coordinates as a tuple.
    pub fn get(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    /// Set X coordinate only.
    pub fn set_x(&mut self, x: i32) {
        self.x = x;
    }

    /// Set Y coordinate only.
    pub fn set_y(&mut self, y: i32) {
        self.y = y;
    }
}

impl Default for TrackXY {
    fn default() -> Self {
        TrackXY::new()
    }
}

impl PartialEq for TrackXY {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl Eq for TrackXY {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_constructor() {
        let tracker = TrackXY::new();
        assert_eq!(tracker.get_x(), 0);
        assert_eq!(tracker.get_y(), 0);
    }

    #[test]
    fn test_with_position() {
        let tracker = TrackXY::with_position(10, 20);
        assert_eq!(tracker.get_x(), 10);
        assert_eq!(tracker.get_y(), 20);
    }

    #[test]
    fn test_set() {
        let mut tracker = TrackXY::new();
        tracker.set(100, 200);
        assert_eq!(tracker.get_x(), 100);
        assert_eq!(tracker.get_y(), 200);
    }

    #[test]
    fn test_set_individual() {
        let mut tracker = TrackXY::new();
        tracker.set_x(50);
        assert_eq!(tracker.get_x(), 50);
        assert_eq!(tracker.get_y(), 0);

        tracker.set_y(75);
        assert_eq!(tracker.get_x(), 50);
        assert_eq!(tracker.get_y(), 75);
    }

    #[test]
    fn test_get_tuple() {
        let tracker = TrackXY::with_position(10, 20);
        assert_eq!(tracker.get(), (10, 20));
    }

    #[test]
    fn test_equality() {
        let t1 = TrackXY::with_position(10, 20);
        let t2 = TrackXY::with_position(10, 20);
        let t3 = TrackXY::with_position(10, 21);
        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    #[test]
    fn test_copy_clone() {
        let t1 = TrackXY::with_position(10, 20);
        let t2 = t1;
        let t3 = t1;
        assert_eq!(t1, t2);
        assert_eq!(t1, t3);
    }
}
