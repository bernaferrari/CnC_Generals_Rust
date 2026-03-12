//! Sequence number wraparound safety checks
//!
//! This module provides validation for sequence numbers to detect and handle
//! wraparound conditions when sequence numbers exceed u16::MAX (65535) and roll
//! over to 0. It also detects gaps, duplicates, and out-of-order sequences.
//!
//! # Example
//!
//! ```
//! use game_network::commands::sequence_validator::{SequenceValidator, SequenceValidationResult};
//!
//! let mut validator = SequenceValidator::new();
//!
//! // Normal sequence
//! assert!(matches!(validator.validate_and_advance(0), SequenceValidationResult::Valid));
//! assert!(matches!(validator.validate_and_advance(1), SequenceValidationResult::Valid));
//!
//! // Wraparound detection
//! validator.validate_and_advance(65534);
//! validator.validate_and_advance(65535);
//! if let SequenceValidationResult::Wraparound { wrap_count } = validator.validate_and_advance(0) {
//!     println!("Detected wraparound! Total wraps: {}", wrap_count);
//! }
//! ```

use tracing::{debug, warn};

/// Gap size threshold for warnings (sequences jumps larger than this trigger warnings)
const LARGE_GAP_THRESHOLD: u16 = 10;

/// Sequence validation results
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceValidationResult {
    /// Sequence is valid and as expected
    Valid,

    /// Sequence is out of order
    OutOfOrder {
        /// Expected sequence number
        expected: u16,
        /// Received sequence number
        got: u16,
    },

    /// Sequence wrapped around from 65535 to 0
    Wraparound {
        /// Number of times sequence has wrapped
        wrap_count: u32,
    },

    /// Gap detected in sequence
    Gap {
        /// Expected sequence number
        expected: u16,
        /// Received sequence number
        got: u16,
        /// Size of the gap
        gap_size: u16,
    },

    /// Duplicate sequence number
    Duplicate,
}

impl Default for SequenceValidationResult {
    fn default() -> Self {
        Self::Valid
    }
}

/// Sequence validator for tracking and validating u16 sequence numbers
/// with wraparound detection and gap analysis
#[derive(Debug, Clone)]
pub struct SequenceValidator {
    /// Last sequence number received
    last_sequence: u16,

    /// Total commands processed
    total_commands: u64,

    /// Number of times sequence has wrapped around
    expected_wraps: u32,

    /// Total command count when last wrap occurred
    last_wrap_at: u64,

    /// Whether overflow detection is enabled
    overflow_detection_enabled: bool,

    /// Whether this is the first sequence received
    first_sequence: bool,
}

impl SequenceValidator {
    /// Create a new sequence validator
    pub fn new() -> Self {
        Self {
            last_sequence: 0,
            total_commands: 0,
            expected_wraps: 0,
            last_wrap_at: 0,
            overflow_detection_enabled: true,
            first_sequence: true,
        }
    }

    /// Validate and advance to the next sequence number
    ///
    /// This method checks the incoming sequence number for:
    /// - Wraparound conditions (65535 -> 0)
    /// - Out-of-order delivery
    /// - Gaps in sequence
    /// - Duplicate sequences
    ///
    /// # Arguments
    ///
    /// * `seq` - The sequence number to validate
    ///
    /// # Returns
    ///
    /// A `SequenceValidationResult` indicating the validation status
    pub fn validate_and_advance(&mut self, seq: u16) -> SequenceValidationResult {
        // Handle first sequence specially
        if self.first_sequence {
            self.last_sequence = seq;
            self.total_commands = 1;
            self.first_sequence = false;
            debug!("First sequence received: {}", seq);
            return SequenceValidationResult::Valid;
        }

        // Check for duplicate
        if seq == self.last_sequence {
            debug!("Duplicate sequence detected: {}", seq);
            return SequenceValidationResult::Duplicate;
        }

        // Calculate expected next sequence
        let expected_next = self.get_expected_next();

        // Check if this is the expected sequence
        if seq == expected_next {
            // Check for wraparound
            if self.overflow_detection_enabled
                && self.last_sequence > seq
                && self.last_sequence >= 65530
                && seq <= 5
            {
                // This is a wraparound condition
                self.expected_wraps += 1;
                self.last_wrap_at = self.total_commands;
                self.last_sequence = seq;
                self.total_commands += 1;

                debug!(
                    "Sequence wraparound detected: {} -> {} (wrap count: {})",
                    self.last_sequence.wrapping_sub(1),
                    seq,
                    self.expected_wraps
                );

                return SequenceValidationResult::Wraparound {
                    wrap_count: self.expected_wraps,
                };
            }

            // Normal valid sequence
            self.last_sequence = seq;
            self.total_commands += 1;
            return SequenceValidationResult::Valid;
        }

        // Check for out-of-order (sequence is less than expected, but not a wraparound)
        if seq < expected_next && !(self.last_sequence >= 65530 && seq <= 5) {
            warn!(
                "Out-of-order sequence detected: expected {}, got {}",
                expected_next, seq
            );
            return SequenceValidationResult::OutOfOrder {
                expected: expected_next,
                got: seq,
            };
        }

        // Check for gap (sequence is ahead of expected)
        if seq > expected_next || (self.last_sequence >= 65530 && seq <= 5 && seq != expected_next)
        {
            // Calculate gap size
            let gap_size = if self.last_sequence >= 65530 && seq <= 5 {
                // Wraparound gap
                (u16::MAX - self.last_sequence) + seq
            } else {
                seq - expected_next
            };

            if gap_size > LARGE_GAP_THRESHOLD {
                warn!(
                    "Large gap detected in sequence: expected {}, got {} (gap: {})",
                    expected_next, seq, gap_size
                );
            } else {
                debug!(
                    "Gap detected in sequence: expected {}, got {} (gap: {})",
                    expected_next, seq, gap_size
                );
            }

            self.last_sequence = seq;
            self.total_commands += 1;

            return SequenceValidationResult::Gap {
                expected: expected_next,
                got: seq,
                gap_size,
            };
        }

        // Update state and return out-of-order
        warn!(
            "Unexpected sequence: expected {}, got {}",
            expected_next, seq
        );

        SequenceValidationResult::OutOfOrder {
            expected: expected_next,
            got: seq,
        }
    }

    /// Check if there are gaps in the sequence
    ///
    /// # Arguments
    ///
    /// * `seq` - The sequence number to check
    ///
    /// # Returns
    ///
    /// `true` if the gap is larger than LARGE_GAP_THRESHOLD
    pub fn check_for_gaps(&self, seq: u16) -> bool {
        if self.first_sequence {
            return false;
        }

        let expected = self.get_expected_next();

        // Check for normal gap
        if seq > expected && (seq - expected) > LARGE_GAP_THRESHOLD {
            return true;
        }

        // Check for wraparound gap
        if self.last_sequence >= 65530 && seq <= 5 {
            let gap = (u16::MAX - self.last_sequence) + seq;
            return gap > LARGE_GAP_THRESHOLD;
        }

        false
    }

    /// Get the expected next sequence number
    pub fn get_expected_next(&self) -> u16 {
        self.last_sequence.wrapping_add(1)
    }

    /// Get the number of times the sequence has wrapped
    pub fn get_wrapped_count(&self) -> u32 {
        self.expected_wraps
    }

    /// Detect abrupt changes in sequence numbers and return a description
    ///
    /// This method identifies large jumps in sequence numbers that might
    /// indicate packet loss, network issues, or other anomalies.
    ///
    /// # Arguments
    ///
    /// * `seq` - The sequence number to check
    ///
    /// # Returns
    ///
    /// An optional description string if an abrupt change is detected
    pub fn detect_abrupt_changes(&mut self, seq: u16) -> Option<String> {
        if self.first_sequence {
            return None;
        }

        let expected = self.get_expected_next();

        // Detect large forward jump
        if seq > expected {
            let gap = seq - expected;
            if gap > 100 {
                return Some(format!(
                    "Abrupt forward jump detected: {} -> {} (gap: {}). Possible packet loss or network disruption.",
                    self.last_sequence, seq, gap
                ));
            } else if gap > 50 {
                return Some(format!(
                    "Significant sequence gap: {} -> {} (gap: {}). Check network stability.",
                    self.last_sequence, seq, gap
                ));
            }
        }

        // Detect backward jump (excluding small out-of-order)
        if seq < expected && (expected - seq) > 10 {
            return Some(format!(
                "Backward sequence jump: {} -> {} (delta: {}). Possible reordering or duplicate packet.",
                self.last_sequence, seq, expected - seq
            ));
        }

        // Detect potential wraparound issues
        if self.last_sequence > 60000 && seq < 100 && seq != expected {
            return Some(format!(
                "Potential wraparound anomaly: {} -> {}. Expected {}, verify sequence continuity.",
                self.last_sequence, seq, expected
            ));
        }

        None
    }

    /// Reset the validator to initial state
    pub fn reset(&mut self) {
        self.last_sequence = 0;
        self.total_commands = 0;
        self.expected_wraps = 0;
        self.last_wrap_at = 0;
        self.first_sequence = true;
        debug!("Sequence validator reset");
    }

    /// Get total commands processed
    pub fn total_commands(&self) -> u64 {
        self.total_commands
    }

    /// Get the last sequence number
    pub fn last_sequence(&self) -> u16 {
        self.last_sequence
    }

    /// Enable or disable overflow detection
    pub fn set_overflow_detection(&mut self, enabled: bool) {
        self.overflow_detection_enabled = enabled;
    }
}

impl Default for SequenceValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_sequence() {
        let mut validator = SequenceValidator::new();

        // Test normal sequence progression
        assert_eq!(
            validator.validate_and_advance(0),
            SequenceValidationResult::Valid
        );
        assert_eq!(
            validator.validate_and_advance(1),
            SequenceValidationResult::Valid
        );
        assert_eq!(
            validator.validate_and_advance(2),
            SequenceValidationResult::Valid
        );
        assert_eq!(
            validator.validate_and_advance(3),
            SequenceValidationResult::Valid
        );

        assert_eq!(validator.total_commands(), 4);
        assert_eq!(validator.get_expected_next(), 4);
    }

    #[test]
    fn test_sequence_wraparound() {
        let mut validator = SequenceValidator::new();

        // Advance to near wraparound point
        validator.validate_and_advance(65533);
        assert_eq!(
            validator.validate_and_advance(65534),
            SequenceValidationResult::Valid
        );
        assert_eq!(
            validator.validate_and_advance(65535),
            SequenceValidationResult::Valid
        );

        // This should detect wraparound
        let result = validator.validate_and_advance(0);
        assert!(matches!(
            result,
            SequenceValidationResult::Wraparound { wrap_count: 1 }
        ));

        assert_eq!(
            validator.validate_and_advance(1),
            SequenceValidationResult::Valid
        );
        assert_eq!(
            validator.validate_and_advance(2),
            SequenceValidationResult::Valid
        );

        assert_eq!(validator.get_wrapped_count(), 1);
    }

    #[test]
    fn test_sequence_gap() {
        let mut validator = SequenceValidator::new();

        assert_eq!(
            validator.validate_and_advance(0),
            SequenceValidationResult::Valid
        );

        // Create a gap by jumping to 10
        let result = validator.validate_and_advance(10);
        if let SequenceValidationResult::Gap {
            expected,
            got,
            gap_size,
        } = result
        {
            assert_eq!(expected, 1);
            assert_eq!(got, 10);
            assert_eq!(gap_size, 9);
        } else {
            panic!("Expected Gap result");
        }

        assert_eq!(validator.get_expected_next(), 11);
    }

    #[test]
    fn test_sequence_out_of_order() {
        let mut validator = SequenceValidator::new();

        assert_eq!(
            validator.validate_and_advance(0),
            SequenceValidationResult::Valid
        );
        assert_eq!(
            validator.validate_and_advance(2),
            SequenceValidationResult::Gap {
                expected: 1,
                got: 2,
                gap_size: 1
            }
        );

        // Now receive sequence 1 (out of order)
        let result = validator.validate_and_advance(1);
        assert!(matches!(
            result,
            SequenceValidationResult::OutOfOrder { .. }
        ));
    }

    #[test]
    fn test_sequence_duplicate() {
        let mut validator = SequenceValidator::new();

        assert_eq!(
            validator.validate_and_advance(0),
            SequenceValidationResult::Valid
        );

        // Send the same sequence again
        assert_eq!(
            validator.validate_and_advance(0),
            SequenceValidationResult::Duplicate
        );

        assert_eq!(
            validator.validate_and_advance(1),
            SequenceValidationResult::Valid
        );
    }

    #[test]
    fn test_multiple_wraps() {
        let mut validator = SequenceValidator::new();

        // Advance through first sequence range
        validator.validate_and_advance(0);
        validator.validate_and_advance(100);
        validator.validate_and_advance(200);

        // Jump to near wraparound
        validator.validate_and_advance(65535);

        // First wrap
        let result = validator.validate_and_advance(0);
        assert!(matches!(
            result,
            SequenceValidationResult::Wraparound { wrap_count: 1 }
        ));

        validator.validate_and_advance(100);

        // Jump to second wraparound
        validator.validate_and_advance(65535);

        // Second wrap
        let result = validator.validate_and_advance(0);
        assert!(matches!(
            result,
            SequenceValidationResult::Wraparound { wrap_count: 2 }
        ));

        assert_eq!(validator.get_wrapped_count(), 2);
    }

    #[test]
    fn test_abrupt_changes() {
        let mut validator = SequenceValidator::new();

        validator.validate_and_advance(0);

        // Test huge jump
        let warning = validator.detect_abrupt_changes(32767);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("Abrupt forward jump"));

        // Actually advance to that sequence
        validator.validate_and_advance(32767);

        // Test backward jump
        let warning = validator.detect_abrupt_changes(100);
        assert!(warning.is_some());
    }

    #[test]
    fn test_check_for_gaps() {
        let mut validator = SequenceValidator::new();

        validator.validate_and_advance(0);

        // Small gap - should return false
        assert!(!validator.check_for_gaps(5));

        // Large gap - should return true
        assert!(validator.check_for_gaps(50));

        // Exact threshold
        assert!(!validator.check_for_gaps(11));
    }

    #[test]
    fn test_statistics() {
        let mut validator = SequenceValidator::new();

        // Add various sequences with gaps, wraps, duplicates
        assert_eq!(
            validator.validate_and_advance(0),
            SequenceValidationResult::Valid
        );
        assert_eq!(
            validator.validate_and_advance(1),
            SequenceValidationResult::Valid
        );

        // Gap
        let result = validator.validate_and_advance(10);
        assert!(matches!(result, SequenceValidationResult::Gap { .. }));

        // Duplicate
        assert_eq!(
            validator.validate_and_advance(10),
            SequenceValidationResult::Duplicate
        );

        // Out of order
        let result = validator.validate_and_advance(5);
        assert!(matches!(
            result,
            SequenceValidationResult::OutOfOrder { .. }
        ));

        // Continue normally
        validator.validate_and_advance(11);

        // Wraparound setup
        validator.validate_and_advance(65535);
        let result = validator.validate_and_advance(0);
        assert!(matches!(
            result,
            SequenceValidationResult::Wraparound { wrap_count: 1 }
        ));

        // Verify statistics
        assert_eq!(validator.get_wrapped_count(), 1);
        assert!(validator.total_commands() > 5);
        assert_eq!(validator.last_sequence(), 0);
    }

    #[test]
    fn test_reset() {
        let mut validator = SequenceValidator::new();

        validator.validate_and_advance(0);
        validator.validate_and_advance(1);
        validator.validate_and_advance(2);

        assert_eq!(validator.total_commands(), 3);

        validator.reset();

        assert_eq!(validator.total_commands(), 0);
        assert_eq!(validator.get_wrapped_count(), 0);
        assert_eq!(validator.last_sequence(), 0);

        // Should work normally after reset
        assert_eq!(
            validator.validate_and_advance(0),
            SequenceValidationResult::Valid
        );
    }
}
