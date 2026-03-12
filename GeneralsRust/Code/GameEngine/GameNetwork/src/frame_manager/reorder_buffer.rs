//! Frame reorder buffer for handling out-of-order frame arrivals
//!
//! This module provides a buffer that can hold out-of-order frames and deliver them
//! when they become ready. This prevents packet loss in high latency/lossy networks
//! where frames might arrive out of order.
//!
//! # Example
//!
//! ```
//! use game_network::frame_manager::{reorder_buffer::FrameReorderBuffer, FrameData};
//!
//! let mut buffer = FrameReorderBuffer::new();
//!
//! // Frame 3 arrives first (out of order)
//! let result = buffer.buffer_frame(FrameData::new(3));
//! assert!(result.is_none()); // Buffered, not ready to deliver
//!
//! // Frame 1 arrives
//! let result = buffer.buffer_frame(FrameData::new(1));
//! // Returns frames 1, 2, 3 if they're all ready
//! ```

use super::FrameData;
use std::collections::HashMap;

/// Maximum number of frames that can be buffered
const MAX_BUFFER_CAPACITY: usize = 50;

/// Frame reorder buffer for handling out-of-order arrivals
///
/// This buffer stores frames that arrive out of order and delivers them
/// when the gap is filled. It maintains a maximum capacity to prevent
/// unbounded growth.
#[derive(Debug)]
pub struct FrameReorderBuffer {
    /// Next expected frame number
    next_expected: u32,

    /// Buffered frames (frame_number -> FrameData)
    buffered_frames: HashMap<u32, FrameData>,

    /// Total number of frames delivered
    delivered_count: u64,
}

impl FrameReorderBuffer {
    /// Create a new frame reorder buffer
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::reorder_buffer::FrameReorderBuffer;
    ///
    /// let buffer = FrameReorderBuffer::new();
    /// assert_eq!(buffer.get_next_expected(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            next_expected: 0,
            buffered_frames: HashMap::with_capacity(MAX_BUFFER_CAPACITY),
            delivered_count: 0,
        }
    }

    /// Check if a frame can be delivered (fills the gap)
    ///
    /// # Arguments
    ///
    /// * `frame_number` - Frame number to check
    ///
    /// # Returns
    ///
    /// true if the frame matches next_expected or fills a gap
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::reorder_buffer::FrameReorderBuffer;
    ///
    /// let buffer = FrameReorderBuffer::new();
    /// assert!(buffer.can_deliver_frame(0)); // Next expected
    /// assert!(!buffer.can_deliver_frame(5)); // Future frame
    /// ```
    pub fn can_deliver_frame(&self, frame_number: u32) -> bool {
        frame_number == self.next_expected
    }

    /// Buffer a frame and return any frames ready to be delivered
    ///
    /// # Arguments
    ///
    /// * `frame` - Frame to buffer
    ///
    /// # Returns
    ///
    /// - `None` if the frame is buffered (not ready to deliver yet)
    /// - `Some(vec![frames...])` if the frame fills a gap - includes this frame + consecutive frames
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::{reorder_buffer::FrameReorderBuffer, FrameData};
    ///
    /// let mut buffer = FrameReorderBuffer::new();
    ///
    /// // Add frame 2 (out of order)
    /// let result = buffer.buffer_frame(FrameData::new(2));
    /// assert!(result.is_none()); // Buffered
    ///
    /// // Add frame 0
    /// let result = buffer.buffer_frame(FrameData::new(0));
    /// assert!(result.is_some()); // Delivers frame 0, 1, 2
    /// ```
    pub fn buffer_frame(&mut self, frame: FrameData) -> Option<Vec<FrameData>> {
        let frame_number = frame.frame_number;

        // Check if this is a duplicate (already delivered or buffered)
        if frame_number < self.next_expected {
            // Frame is in the past (already delivered)
            return Some(Vec::new()); // Return empty vec to indicate processed but not delivered
        }

        if self.buffered_frames.contains_key(&frame_number) {
            // Frame is already buffered (duplicate)
            return Some(Vec::new()); // Return empty vec to indicate processed but not delivered
        }

        // Check if this frame can be delivered immediately
        if frame_number == self.next_expected {
            // This frame is next in sequence
            let mut frames_to_deliver = vec![frame];
            self.next_expected = self.next_expected.wrapping_add(1);
            self.delivered_count += 1;

            // Check if we can also deliver buffered frames
            while let Some(buffered_frame) = self.buffered_frames.remove(&self.next_expected) {
                frames_to_deliver.push(buffered_frame);
                self.next_expected = self.next_expected.wrapping_add(1);
                self.delivered_count += 1;
            }

            return Some(frames_to_deliver);
        }

        // Frame is out of order, buffer it if we have capacity
        if self.buffered_frames.len() < MAX_BUFFER_CAPACITY {
            self.buffered_frames.insert(frame_number, frame);
            None // Buffered, not ready to deliver
        } else {
            // Buffer is full, check if we should drop this frame or oldest buffered frame
            // Drop frames that are too far ahead (beyond buffer capacity)
            let max_acceptable_frame = self.next_expected + MAX_BUFFER_CAPACITY as u32;

            if frame_number >= max_acceptable_frame {
                // Frame is too far ahead, drop it
                #[cfg(feature = "metrics")]
                tracing::warn!(
                    "Dropping frame {} - too far ahead (next_expected: {}, capacity: {})",
                    frame_number,
                    self.next_expected,
                    MAX_BUFFER_CAPACITY
                );
                Some(Vec::new()) // Return empty vec to indicate processed but dropped
            } else {
                // Buffer is full, but this frame is within acceptable range
                // Find and remove the oldest frame (furthest ahead)
                if let Some(max_frame_num) = self.buffered_frames.keys().max().copied() {
                    if frame_number < max_frame_num {
                        // This frame is closer than the furthest buffered frame
                        self.buffered_frames.remove(&max_frame_num);
                        self.buffered_frames.insert(frame_number, frame);
                        #[cfg(feature = "metrics")]
                        tracing::warn!(
                            "Buffer full - dropped frame {} to make room for frame {}",
                            max_frame_num,
                            frame_number
                        );
                    }
                }
                None // Buffered or dropped
            }
        }
    }

    /// Get the next expected frame number
    ///
    /// # Returns
    ///
    /// The frame number that should arrive next in sequence
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::{reorder_buffer::FrameReorderBuffer, FrameData};
    ///
    /// let mut buffer = FrameReorderBuffer::new();
    /// assert_eq!(buffer.get_next_expected(), 0);
    ///
    /// buffer.buffer_frame(FrameData::new(0));
    /// assert_eq!(buffer.get_next_expected(), 1);
    /// ```
    pub fn get_next_expected(&self) -> u32 {
        self.next_expected
    }

    /// Get the number of buffered frames
    ///
    /// # Returns
    ///
    /// Number of frames currently in the buffer
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::{reorder_buffer::FrameReorderBuffer, FrameData};
    ///
    /// let mut buffer = FrameReorderBuffer::new();
    /// assert_eq!(buffer.len(), 0);
    ///
    /// buffer.buffer_frame(FrameData::new(2));
    /// assert_eq!(buffer.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.buffered_frames.len()
    }

    /// Check if the buffer is empty
    ///
    /// # Returns
    ///
    /// true if no frames are buffered
    pub fn is_empty(&self) -> bool {
        self.buffered_frames.is_empty()
    }

    /// Clear the buffer and reset to initial state
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::{reorder_buffer::FrameReorderBuffer, FrameData};
    ///
    /// let mut buffer = FrameReorderBuffer::new();
    /// buffer.buffer_frame(FrameData::new(2));
    ///
    /// buffer.clear();
    /// assert_eq!(buffer.len(), 0);
    /// assert_eq!(buffer.get_next_expected(), 0);
    /// ```
    pub fn clear(&mut self) {
        self.buffered_frames.clear();
        self.next_expected = 0;
        self.delivered_count = 0;
    }

    /// Get statistics about the reorder buffer
    ///
    /// # Returns
    ///
    /// Tuple of (buffered_count, delivered_count)
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::{reorder_buffer::FrameReorderBuffer, FrameData};
    ///
    /// let mut buffer = FrameReorderBuffer::new();
    /// buffer.buffer_frame(FrameData::new(0));
    ///
    /// let (buffered, delivered) = buffer.stats();
    /// assert_eq!(delivered, 1);
    /// ```
    pub fn stats(&self) -> (usize, u64) {
        (self.buffered_frames.len(), self.delivered_count)
    }
}

impl Default for FrameReorderBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_order_delivery() {
        let mut buffer = FrameReorderBuffer::new();

        // Add frames 0, 1, 2 in order
        let result = buffer.buffer_frame(FrameData::new(0));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame_number, 0);
        assert_eq!(buffer.len(), 0);

        let result = buffer.buffer_frame(FrameData::new(1));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame_number, 1);
        assert_eq!(buffer.len(), 0);

        let result = buffer.buffer_frame(FrameData::new(2));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame_number, 2);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_out_of_order_buffering() {
        let mut buffer = FrameReorderBuffer::new();

        // Add frame 2 (out of order)
        let result = buffer.buffer_frame(FrameData::new(2));
        assert!(result.is_none()); // Buffered
        assert_eq!(buffer.len(), 1);

        // Add frame 0
        let result = buffer.buffer_frame(FrameData::new(0));
        assert!(result.is_some());
        let frames = result.unwrap();
        // Should deliver frame 0, 1 is still missing, so only frame 0
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame_number, 0);
        assert_eq!(buffer.len(), 1); // Frame 2 still buffered

        // Add frame 1
        let result = buffer.buffer_frame(FrameData::new(1));
        assert!(result.is_some());
        let frames = result.unwrap();
        // Should deliver frames 1 and 2 together
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_number, 1);
        assert_eq!(frames[1].frame_number, 2);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_partial_out_of_order() {
        let mut buffer = FrameReorderBuffer::new();

        // Add frame 0 (deliver immediately)
        let result = buffer.buffer_frame(FrameData::new(0));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame_number, 0);

        // Add frame 2 (buffer, waiting for 1)
        let result = buffer.buffer_frame(FrameData::new(2));
        assert!(result.is_none());
        assert_eq!(buffer.len(), 1);

        // Add frame 1 (deliver 1 and 2 together)
        let result = buffer.buffer_frame(FrameData::new(1));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_number, 1);
        assert_eq!(frames[1].frame_number, 2);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_buffer_overflow() {
        let mut buffer = FrameReorderBuffer::new();

        // Add frame 0 to establish next_expected = 1
        let result = buffer.buffer_frame(FrameData::new(0));
        assert!(result.is_some());

        // Add frame 2 (buffer)
        let result = buffer.buffer_frame(FrameData::new(2));
        assert!(result.is_none());
        assert_eq!(buffer.len(), 1);

        // Add frames 3-51 (should fill buffer to capacity)
        for i in 3..52 {
            let result = buffer.buffer_frame(FrameData::new(i));
            if i < 51 {
                assert!(result.is_none(), "Frame {} should be buffered", i);
            }
        }

        // Buffer should be at capacity (50 frames)
        assert_eq!(buffer.len(), MAX_BUFFER_CAPACITY);

        // Add frames 100-110 (far ahead, should be dropped)
        for i in 100..111 {
            let result = buffer.buffer_frame(FrameData::new(i));
            // Should return Some(empty vec) indicating dropped
            assert!(result.is_some(), "Frame {} should be processed", i);
            if let Some(frames) = result {
                assert_eq!(frames.len(), 0, "Frame {} should be dropped", i);
            }
        }

        // Buffer should still be at capacity
        assert_eq!(buffer.len(), MAX_BUFFER_CAPACITY);

        // When frame 1 arrives, should deliver 1, 2, 3, ... up to first gap
        let result = buffer.buffer_frame(FrameData::new(1));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert!(frames.len() > 0);
        assert_eq!(frames[0].frame_number, 1);
    }

    #[test]
    fn test_duplicate_in_buffer() {
        let mut buffer = FrameReorderBuffer::new();

        // Add frame 2 (buffer)
        let result = buffer.buffer_frame(FrameData::new(2));
        assert!(result.is_none());
        assert_eq!(buffer.len(), 1);

        // Add frame 2 again (duplicate, should be ignored)
        let result = buffer.buffer_frame(FrameData::new(2));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 0); // Empty vec indicates duplicate
        assert_eq!(buffer.len(), 1); // Still only 1 buffered

        // Add frame 0 (deliver)
        let result = buffer.buffer_frame(FrameData::new(0));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame_number, 0);

        // Add frame 1 (deliver 1 and 2)
        let result = buffer.buffer_frame(FrameData::new(1));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_number, 1);
        assert_eq!(frames[1].frame_number, 2);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_get_next_expected() {
        let mut buffer = FrameReorderBuffer::new();
        assert_eq!(buffer.get_next_expected(), 0);

        buffer.buffer_frame(FrameData::new(0));
        assert_eq!(buffer.get_next_expected(), 1);

        buffer.buffer_frame(FrameData::new(1));
        assert_eq!(buffer.get_next_expected(), 2);
    }

    #[test]
    fn test_len() {
        let mut buffer = FrameReorderBuffer::new();
        assert_eq!(buffer.len(), 0);

        buffer.buffer_frame(FrameData::new(2));
        assert_eq!(buffer.len(), 1);

        buffer.buffer_frame(FrameData::new(3));
        assert_eq!(buffer.len(), 2);

        buffer.buffer_frame(FrameData::new(0));
        buffer.buffer_frame(FrameData::new(1));
        assert_eq!(buffer.len(), 0); // All delivered
    }

    #[test]
    fn test_clear() {
        let mut buffer = FrameReorderBuffer::new();

        buffer.buffer_frame(FrameData::new(0));
        buffer.buffer_frame(FrameData::new(2));

        buffer.clear();

        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.get_next_expected(), 0);
        let (buffered, delivered) = buffer.stats();
        assert_eq!(buffered, 0);
        assert_eq!(delivered, 0);
    }

    #[test]
    fn test_stats() {
        let mut buffer = FrameReorderBuffer::new();

        let (buffered, delivered) = buffer.stats();
        assert_eq!(buffered, 0);
        assert_eq!(delivered, 0);

        buffer.buffer_frame(FrameData::new(0));
        let (buffered, delivered) = buffer.stats();
        assert_eq!(buffered, 0);
        assert_eq!(delivered, 1);

        buffer.buffer_frame(FrameData::new(2));
        let (buffered, delivered) = buffer.stats();
        assert_eq!(buffered, 1);
        assert_eq!(delivered, 1);
    }

    #[test]
    fn test_can_deliver_frame() {
        let buffer = FrameReorderBuffer::new();
        assert!(buffer.can_deliver_frame(0));
        assert!(!buffer.can_deliver_frame(1));
        assert!(!buffer.can_deliver_frame(100));
    }

    #[test]
    fn test_wraparound() {
        let mut buffer = FrameReorderBuffer::new();
        buffer.next_expected = u32::MAX - 2;

        // Add frames near wraparound
        let result = buffer.buffer_frame(FrameData::new(u32::MAX - 2));
        assert!(result.is_some());
        assert_eq!(buffer.get_next_expected(), u32::MAX - 1);

        let result = buffer.buffer_frame(FrameData::new(u32::MAX - 1));
        assert!(result.is_some());
        assert_eq!(buffer.get_next_expected(), u32::MAX);

        let result = buffer.buffer_frame(FrameData::new(u32::MAX));
        assert!(result.is_some());
        assert_eq!(buffer.get_next_expected(), 0); // Wrapped around

        let result = buffer.buffer_frame(FrameData::new(0));
        assert!(result.is_some());
        assert_eq!(buffer.get_next_expected(), 1);
    }

    #[test]
    fn test_past_frame_handling() {
        let mut buffer = FrameReorderBuffer::new();

        // Deliver frames 0, 1, 2
        buffer.buffer_frame(FrameData::new(0));
        buffer.buffer_frame(FrameData::new(1));
        buffer.buffer_frame(FrameData::new(2));

        assert_eq!(buffer.get_next_expected(), 3);

        // Try to add frame 0 again (in the past)
        let result = buffer.buffer_frame(FrameData::new(0));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 0); // Empty vec indicates already processed

        // Try to add frame 1 again (in the past)
        let result = buffer.buffer_frame(FrameData::new(1));
        assert!(result.is_some());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 0); // Empty vec indicates already processed
    }
}
