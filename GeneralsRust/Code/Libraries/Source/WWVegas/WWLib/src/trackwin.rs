//! Window tracking mirroring WWLib `trackwin.h`.
//!
//! This class is used to keep track of a subwindow within a larger window.
//! Note: The C++ implementation is wrapped in `#ifdef NEVER`, meaning it was
//! disabled in the original codebase. We provide the full implementation for
//! potential future use while maintaining the same interface.

use crate::point::TPoint2D;

/// Rectangle type for window tracking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// X coordinate of upper-left corner
    pub x: i32,
    /// Y coordinate of upper-left corner
    pub y: i32,
    /// Width of rectangle
    pub width: i32,
    /// Height of rectangle
    pub height: i32,
}

impl Rect {
    /// Create a new rectangle.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if rectangle is valid (width and height > 0).
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Get the size (area) of the rectangle.
    pub fn size(&self) -> i32 {
        self.width * self.height
    }

    /// Get the top-left point.
    pub fn top_left(&self) -> TPoint2D<i32> {
        TPoint2D::new(self.x, self.y)
    }

    /// Get the top-right point.
    pub fn top_right(&self) -> TPoint2D<i32> {
        TPoint2D::new(self.x + self.width - 1, self.y)
    }

    /// Get the bottom-left point.
    pub fn bottom_left(&self) -> TPoint2D<i32> {
        TPoint2D::new(self.x, self.y + self.height - 1)
    }

    /// Get the bottom-right point.
    pub fn bottom_right(&self) -> TPoint2D<i32> {
        TPoint2D::new(self.x + self.width - 1, self.y + self.height - 1)
    }

    /// Determine if a point lies within the rectangle.
    pub fn contains_point(&self, point: &TPoint2D<i32>) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width
            && point.y >= self.y
            && point.y < self.y + self.height
    }

    /// Determine if two rectangles overlap.
    pub fn is_overlapping(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.y < other.y + other.height
            && self.x + self.width > other.x
            && self.y + self.height > other.y
    }
}

impl Default for Rect {
    fn default() -> Self {
        Rect::new(0, 0, 0, 0)
    }
}

/// Tracks a subwindow within a larger window.
///
/// This class is used to keep track of a subwindow within a larger window.
#[derive(Clone, Copy, Debug)]
pub struct TrackWindow {
    /// The sub-window dimensions.
    window: Rect,

    /// This holds the full sized window. It is used for resetting the window
    /// and for maximum window dimension clipping.
    full_window: Rect,
}

impl TrackWindow {
    /// Create a new TrackWindow with the specified dimensions.
    ///
    /// Matches C++ `TrackWindow(int width, int height) : Window(0, 0, width, height), FullWindow(0, 0, width, height) {}`.
    pub fn new(width: i32, height: i32) -> Self {
        let rect = Rect::new(0, 0, width, height);
        TrackWindow {
            window: rect,
            full_window: rect,
        }
    }

    /// Create a TrackWindow from a rectangle.
    pub fn from_rect(rect: Rect) -> Self {
        TrackWindow {
            window: rect,
            full_window: rect,
        }
    }

    /// Set the sub-window to a new rectangle.
    ///
    /// Matches C++ `void Set(Rect const & rect) {Window = rect; if (FullWindow.Width==0) FullWindow = rect;}`.
    pub fn set(&mut self, rect: Rect) {
        self.window = rect;
        if self.full_window.width == 0 {
            self.full_window = rect;
        }
    }

    /// Reset the sub-window to the full window dimensions.
    ///
    /// Matches C++ `void Reset(void) {Window = Full_Rect();}`.
    pub fn reset(&mut self) {
        self.window = self.full_window;
    }

    /// Get the X coordinate of the sub-window.
    ///
    /// Matches C++ `int Get_X(void) const {return(Window.X);}`.
    pub fn get_x(&self) -> i32 {
        self.window.x
    }

    /// Get the Y coordinate of the sub-window.
    ///
    /// Matches C++ `int Get_Y(void) const {return(Window.Y);}`.
    pub fn get_y(&self) -> i32 {
        self.window.y
    }

    /// Get the width of the sub-window.
    ///
    /// Matches C++ `int Get_Width(void) const {return(Window.Width);}`.
    pub fn get_width(&self) -> i32 {
        self.window.width
    }

    /// Get the height of the sub-window.
    ///
    /// Matches C++ `int Get_Height(void) const {return(Window.Height);}`.
    pub fn get_height(&self) -> i32 {
        self.window.height
    }

    /// Get the sub-window rectangle.
    ///
    /// Matches C++ `Rect Get_Rect(void) const {return(Window);}`.
    pub fn get_rect(&self) -> Rect {
        self.window
    }

    /// Get the full window width.
    ///
    /// Matches C++ `int Full_Width(void) const {return(FullWindow.Width);}`.
    pub fn full_width(&self) -> i32 {
        self.full_window.width
    }

    /// Get the full window height.
    ///
    /// Matches C++ `int Full_Height(void) const {return(FullWindow.Height);}`.
    pub fn full_height(&self) -> i32 {
        self.full_window.height
    }

    /// Get the full window rectangle.
    ///
    /// Matches C++ `Rect Full_Rect(void) const {return(FullWindow);}`.
    pub fn full_rect(&self) -> Rect {
        self.full_window
    }
}

impl PartialEq for TrackWindow {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window && self.full_window == other.full_window
    }
}

impl Eq for TrackWindow {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_creation() {
        let rect = Rect::new(10, 20, 100, 200);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 200);
    }

    #[test]
    fn test_rect_is_valid() {
        let valid = Rect::new(0, 0, 10, 10);
        assert!(valid.is_valid());

        let invalid_w = Rect::new(0, 0, 0, 10);
        assert!(!invalid_w.is_valid());

        let invalid_h = Rect::new(0, 0, 10, 0);
        assert!(!invalid_h.is_valid());
    }

    #[test]
    fn test_rect_contains_point() {
        let rect = Rect::new(10, 10, 100, 100);
        let inside = TPoint2D::new(50, 50);
        let outside = TPoint2D::new(5, 5);
        let edge = TPoint2D::new(10, 10); // Top-left corner is inside
        let past_edge = TPoint2D::new(110, 50); // Past right edge

        assert!(rect.contains_point(&inside));
        assert!(!rect.contains_point(&outside));
        assert!(rect.contains_point(&edge));
        assert!(!rect.contains_point(&past_edge));
    }

    #[test]
    fn test_rect_overlapping() {
        let rect1 = Rect::new(0, 0, 100, 100);
        let rect2 = Rect::new(50, 50, 100, 100);
        let rect3 = Rect::new(200, 200, 100, 100);

        assert!(rect1.is_overlapping(&rect2));
        assert!(!rect1.is_overlapping(&rect3));
    }

    #[test]
    fn test_track_window_creation() {
        let tw = TrackWindow::new(640, 480);
        assert_eq!(tw.get_x(), 0);
        assert_eq!(tw.get_y(), 0);
        assert_eq!(tw.get_width(), 640);
        assert_eq!(tw.get_height(), 480);
        assert_eq!(tw.full_width(), 640);
        assert_eq!(tw.full_height(), 480);
    }

    #[test]
    fn test_track_window_set() {
        let mut tw = TrackWindow::new(640, 480);
        let new_rect = Rect::new(10, 20, 300, 200);
        tw.set(new_rect);

        assert_eq!(tw.get_x(), 10);
        assert_eq!(tw.get_y(), 20);
        assert_eq!(tw.get_width(), 300);
        assert_eq!(tw.get_height(), 200);
    }

    #[test]
    fn test_track_window_reset() {
        let mut tw = TrackWindow::new(640, 480);
        tw.set(Rect::new(10, 20, 300, 200));
        tw.reset();

        assert_eq!(tw.get_rect(), tw.full_rect());
        assert_eq!(tw.get_x(), 0);
        assert_eq!(tw.get_y(), 0);
        assert_eq!(tw.get_width(), 640);
        assert_eq!(tw.get_height(), 480);
    }

    #[test]
    fn test_track_window_set_preserves_full_when_width_nonzero() {
        let mut tw = TrackWindow::new(640, 480);
        let original_full = tw.full_rect();

        // Setting a new rect should NOT change full_window since width != 0
        tw.set(Rect::new(10, 20, 300, 200));
        assert_eq!(tw.full_rect(), original_full);
    }

    #[test]
    fn test_rect_points() {
        let rect = Rect::new(10, 20, 100, 200);
        assert_eq!(rect.top_left(), TPoint2D::new(10, 20));
        assert_eq!(rect.top_right(), TPoint2D::new(109, 20));
        assert_eq!(rect.bottom_left(), TPoint2D::new(10, 219));
        assert_eq!(rect.bottom_right(), TPoint2D::new(109, 219));
    }
}
