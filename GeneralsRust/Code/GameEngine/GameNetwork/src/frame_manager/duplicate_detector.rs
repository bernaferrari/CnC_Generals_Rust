//! Frame Duplicate Detection Module
//!
//! This module provides duplicate detection for game frames to prevent processing
//! the same frame multiple times. It tracks recently seen frames using a circular
//! buffer approach, storing frame number and CRC combinations.
//!
//! # Architecture
//!
//! The duplicate detector maintains a sliding window of the last N frames seen,
//! storing their frame numbers and CRC values. This allows efficient detection of:
//! - Exact duplicates (same frame number + same CRC)
//! - Corrupted retransmissions (same frame number + different CRC)
//!
//! # Example
//!
//! ```
//! use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
//!
//! let mut detector = FrameDuplicateDetector::new(50);
//!
//! // First time seeing frame 1
//! assert!(!detector.is_duplicate(1, 0x12345678));
//! detector.add_frame(1, 0x12345678);
//!
//! // Second time seeing frame 1 (duplicate)
//! assert!(detector.is_duplicate(1, 0x12345678));
//! ```

use std::collections::VecDeque;

/// Default capacity for the duplicate detection buffer
pub const DEFAULT_DUPLICATE_BUFFER_SIZE: usize = 50;

/// Frame identifier combining frame number and CRC checksum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameIdentifier {
    /// Frame number
    frame_number: u32,
    /// CRC checksum for the frame
    crc: u32,
}

/// Frame Duplicate Detector
///
/// Tracks recently seen frames to detect duplicates. Uses a circular buffer
/// to maintain a sliding window of frame identifiers.
#[derive(Debug)]
pub struct FrameDuplicateDetector {
    /// Circular buffer of recently seen frames
    seen_frames: VecDeque<FrameIdentifier>,

    /// Maximum number of frames to track
    capacity: usize,

    /// Whether duplicate detection is enabled
    enabled: bool,
}

impl FrameDuplicateDetector {
    /// Create a new duplicate detector with specified capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of frames to track in the buffer
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
    ///
    /// let detector = FrameDuplicateDetector::new(50);
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            seen_frames: VecDeque::with_capacity(capacity),
            capacity,
            enabled: true,
        }
    }

    /// Create a new duplicate detector with default capacity
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
    ///
    /// let detector = FrameDuplicateDetector::default();
    /// ```
    pub fn default() -> Self {
        Self::new(DEFAULT_DUPLICATE_BUFFER_SIZE)
    }

    /// Create a disabled duplicate detector (passes all frames)
    ///
    /// Useful for testing or scenarios where duplicate detection should be bypassed.
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
    ///
    /// let detector = FrameDuplicateDetector::disabled();
    /// assert!(!detector.is_duplicate(1, 0x12345678)); // Never returns true
    /// ```
    pub fn disabled() -> Self {
        Self {
            seen_frames: VecDeque::new(),
            capacity: 0,
            enabled: false,
        }
    }

    /// Check if a frame is a duplicate
    ///
    /// A frame is considered a duplicate if it has the same frame number as
    /// any recently seen frame, regardless of CRC. This catches both exact
    /// duplicates and corrupted retransmissions.
    ///
    /// # Arguments
    ///
    /// * `frame_number` - The frame number to check
    /// * `crc` - The CRC checksum of the frame (unused but kept for API consistency)
    ///
    /// # Returns
    ///
    /// `true` if this frame number has been seen before, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
    ///
    /// let mut detector = FrameDuplicateDetector::new(50);
    ///
    /// detector.add_frame(1, 0x12345678);
    ///
    /// // Exact duplicate (same frame + same CRC)
    /// assert!(detector.is_duplicate(1, 0x12345678));
    ///
    /// // Corrupted retransmission (same frame + different CRC)
    /// assert!(detector.is_duplicate(1, 0xABCDEF00));
    ///
    /// // Different frame
    /// assert!(!detector.is_duplicate(2, 0x12345678));
    /// ```
    pub fn is_duplicate(&self, frame_number: u32, _crc: u32) -> bool {
        if !self.enabled {
            return false;
        }

        // Check if we've seen this frame number before
        self.seen_frames
            .iter()
            .any(|f| f.frame_number == frame_number)
    }

    /// Add a frame to the tracking buffer
    ///
    /// Stores the frame number and CRC in the circular buffer. If the buffer
    /// is full, the oldest entry is removed.
    ///
    /// # Arguments
    ///
    /// * `frame_number` - The frame number to add
    /// * `crc` - The CRC checksum of the frame
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
    ///
    /// let mut detector = FrameDuplicateDetector::new(50);
    /// detector.add_frame(1, 0x12345678);
    /// ```
    pub fn add_frame(&mut self, frame_number: u32, crc: u32) {
        if !self.enabled {
            return;
        }

        // Create new frame identifier
        let frame_id = FrameIdentifier { frame_number, crc };

        // Add to buffer
        self.seen_frames.push_back(frame_id);

        // Maintain capacity by removing oldest if needed
        if self.seen_frames.len() > self.capacity {
            self.seen_frames.pop_front();
        }
    }

    /// Enable duplicate detection
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
    ///
    /// let mut detector = FrameDuplicateDetector::disabled();
    /// detector.enable();
    /// ```
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable duplicate detection
    ///
    /// When disabled, `is_duplicate()` always returns `false` and `add_frame()`
    /// does nothing.
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
    ///
    /// let mut detector = FrameDuplicateDetector::new(50);
    /// detector.disable();
    /// assert!(!detector.is_duplicate(1, 0x12345678)); // Always false when disabled
    /// ```
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check if duplicate detection is enabled
    ///
    /// # Returns
    ///
    /// `true` if detection is enabled, `false` otherwise
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Clear all tracked frames
    ///
    /// Removes all entries from the tracking buffer, useful for resetting
    /// the detector state.
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::duplicate_detector::FrameDuplicateDetector;
    ///
    /// let mut detector = FrameDuplicateDetector::new(50);
    /// detector.add_frame(1, 0x12345678);
    /// detector.clear();
    /// assert!(!detector.is_duplicate(1, 0x12345678)); // No longer a duplicate
    /// ```
    pub fn clear(&mut self) {
        self.seen_frames.clear();
    }

    /// Get the number of frames currently being tracked
    ///
    /// # Returns
    ///
    /// Number of frames in the tracking buffer
    pub fn len(&self) -> usize {
        self.seen_frames.len()
    }

    /// Check if the tracking buffer is empty
    ///
    /// # Returns
    ///
    /// `true` if no frames are being tracked, `false` otherwise
    pub fn is_empty(&self) -> bool {
        self.seen_frames.is_empty()
    }

    /// Get the capacity of the tracking buffer
    ///
    /// # Returns
    ///
    /// Maximum number of frames that can be tracked
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let detector = FrameDuplicateDetector::new(50);
        assert_eq!(detector.capacity(), 50);
        assert!(detector.is_enabled());
        assert!(detector.is_empty());
    }

    #[test]
    fn test_default_detector() {
        let detector = FrameDuplicateDetector::default();
        assert_eq!(detector.capacity(), DEFAULT_DUPLICATE_BUFFER_SIZE);
        assert!(detector.is_enabled());
    }

    #[test]
    fn test_disabled_detector() {
        let detector = FrameDuplicateDetector::disabled();
        assert!(!detector.is_enabled());
        assert!(!detector.is_duplicate(1, 0x12345678));
    }

    #[test]
    fn test_basic_duplicate_detection() {
        let mut detector = FrameDuplicateDetector::new(50);

        // First occurrence - not a duplicate
        assert!(!detector.is_duplicate(1, 0x12345678));

        // Add the frame
        detector.add_frame(1, 0x12345678);

        // Second occurrence - is a duplicate
        assert!(detector.is_duplicate(1, 0x12345678));
    }

    #[test]
    fn test_duplicate_different_crc() {
        let mut detector = FrameDuplicateDetector::new(50);

        // Add frame 1 with CRC X
        detector.add_frame(1, 0x12345678);

        // Same frame number, different CRC - still a duplicate
        assert!(detector.is_duplicate(1, 0xABCDEF00));
    }

    #[test]
    fn test_different_frame_not_duplicate() {
        let mut detector = FrameDuplicateDetector::new(50);

        // Add frame 1 with CRC X
        detector.add_frame(1, 0x12345678);

        // Different frame number, same CRC - not a duplicate
        assert!(!detector.is_duplicate(2, 0x12345678));
    }

    #[test]
    fn test_circular_buffer_behavior() {
        let mut detector = FrameDuplicateDetector::new(3);

        // Add 4 frames (exceeding capacity)
        detector.add_frame(1, 0x1111);
        detector.add_frame(2, 0x2222);
        detector.add_frame(3, 0x3333);
        detector.add_frame(4, 0x4444);

        // Buffer should maintain capacity of 3
        assert_eq!(detector.len(), 3);

        // Frame 1 should have been evicted (oldest)
        assert!(!detector.is_duplicate(1, 0x1111));

        // Frames 2, 3, 4 should still be tracked
        assert!(detector.is_duplicate(2, 0x2222));
        assert!(detector.is_duplicate(3, 0x3333));
        assert!(detector.is_duplicate(4, 0x4444));
    }

    #[test]
    fn test_enable_disable() {
        let mut detector = FrameDuplicateDetector::new(50);

        detector.add_frame(1, 0x12345678);
        assert!(detector.is_duplicate(1, 0x12345678));

        // Disable detection
        detector.disable();
        assert!(!detector.is_enabled());
        assert!(!detector.is_duplicate(1, 0x12345678)); // Returns false when disabled

        // Re-enable detection
        detector.enable();
        assert!(detector.is_enabled());
        assert!(detector.is_duplicate(1, 0x12345678)); // Returns true again
    }

    #[test]
    fn test_clear() {
        let mut detector = FrameDuplicateDetector::new(50);

        detector.add_frame(1, 0x12345678);
        detector.add_frame(2, 0xABCDEF00);
        assert_eq!(detector.len(), 2);

        detector.clear();
        assert_eq!(detector.len(), 0);
        assert!(detector.is_empty());

        // Previously seen frames are no longer duplicates
        assert!(!detector.is_duplicate(1, 0x12345678));
        assert!(!detector.is_duplicate(2, 0xABCDEF00));
    }

    #[test]
    fn test_multiple_frames_sequence() {
        let mut detector = FrameDuplicateDetector::new(50);

        // Add frames in sequence
        for i in 1..=10 {
            assert!(!detector.is_duplicate(i, i as u32 * 0x1000));
            detector.add_frame(i, i as u32 * 0x1000);
            assert!(detector.is_duplicate(i, i as u32 * 0x1000));
        }

        assert_eq!(detector.len(), 10);
    }

    #[test]
    fn test_disabled_does_not_track() {
        let mut detector = FrameDuplicateDetector::disabled();

        detector.add_frame(1, 0x12345678);
        assert_eq!(detector.len(), 0); // Should not add when disabled
    }
}
