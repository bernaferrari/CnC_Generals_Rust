//! Lookup table utilities for fast function evaluation.
//!
//! This module provides structures for creating and using lookup tables
//! to approximate continuous functions with discrete sampled values.
//! This is useful for expensive functions that need to be evaluated
//! frequently during runtime.

use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex};

/// A 1D curve trait for functions that can be sampled to create lookup tables.
///
/// This trait represents a continuous function y = f(x) that can be evaluated
/// at arbitrary points and has a finite domain.
pub trait Curve1D {
    /// Evaluate the curve at a given x value.
    ///
    /// # Arguments
    /// * `x` - Input value
    ///
    /// # Returns
    /// Output value y = f(x)
    fn evaluate(&self, x: f32) -> f32;

    /// Get the domain of the curve.
    ///
    /// # Returns
    /// Tuple of (min_x, max_x)
    fn domain(&self) -> (f32, f32);
}

/// A linear curve implementation for testing and simple cases.
#[derive(Debug, Clone)]
pub struct LinearCurve {
    /// Slope of the line
    pub slope: f32,
    /// Y-intercept
    pub intercept: f32,
    /// Minimum x value
    pub min_x: f32,
    /// Maximum x value
    pub max_x: f32,
}

impl LinearCurve {
    /// Create a new linear curve.
    ///
    /// # Arguments
    /// * `slope` - Slope of the line
    /// * `intercept` - Y-intercept
    /// * `min_x` - Minimum x value
    /// * `max_x` - Maximum x value
    pub fn new(slope: f32, intercept: f32, min_x: f32, max_x: f32) -> Self {
        Self {
            slope,
            intercept,
            min_x,
            max_x,
        }
    }

    /// Create a linear curve from two points.
    ///
    /// # Arguments
    /// * `x1` - X coordinate of first point
    /// * `y1` - Y coordinate of first point
    /// * `x2` - X coordinate of second point
    /// * `y2` - Y coordinate of second point
    pub fn from_points(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        let slope = (y2 - y1) / (x2 - x1);
        let intercept = y1 - slope * x1;
        Self::new(slope, intercept, x1.min(x2), x1.max(x2))
    }
}

impl Curve1D for LinearCurve {
    fn evaluate(&self, x: f32) -> f32 {
        self.slope * x + self.intercept
    }

    fn domain(&self) -> (f32, f32) {
        (self.min_x, self.max_x)
    }
}

/// A sine curve implementation for testing.
#[derive(Debug, Clone)]
pub struct SineCurve {
    /// Amplitude
    pub amplitude: f32,
    /// Frequency
    pub frequency: f32,
    /// Phase offset
    pub phase: f32,
    /// Vertical offset
    pub offset: f32,
    /// Minimum x value
    pub min_x: f32,
    /// Maximum x value
    pub max_x: f32,
}

impl SineCurve {
    /// Create a new sine curve.
    ///
    /// # Arguments
    /// * `amplitude` - Amplitude of the sine wave
    /// * `frequency` - Frequency of the sine wave
    /// * `phase` - Phase offset in radians
    /// * `offset` - Vertical offset
    /// * `min_x` - Minimum x value
    /// * `max_x` - Maximum x value
    pub fn new(
        amplitude: f32,
        frequency: f32,
        phase: f32,
        offset: f32,
        min_x: f32,
        max_x: f32,
    ) -> Self {
        Self {
            amplitude,
            frequency,
            phase,
            offset,
            min_x,
            max_x,
        }
    }
}

impl Curve1D for SineCurve {
    fn evaluate(&self, x: f32) -> f32 {
        self.amplitude * (self.frequency * x + self.phase).sin() + self.offset
    }

    fn domain(&self) -> (f32, f32) {
        (self.min_x, self.max_x)
    }
}

/// A lookup table that provides fast approximation of a continuous function.
///
/// The table stores precomputed samples of the function and uses linear
/// interpolation to approximate values between samples.
#[derive(Debug, Clone)]
pub struct LookupTable {
    /// Name of the table
    pub name: String,
    /// Minimum input value
    min_input: f32,
    /// Maximum input value
    max_input: f32,
    /// Reciprocal of (max - min) for fast normalization
    inv_range: f32,
    /// Sampled output values
    samples: Vec<f32>,
}

impl LookupTable {
    /// Create a new lookup table from a curve.
    ///
    /// # Arguments
    /// * `name` - Name of the table
    /// * `curve` - Curve to sample
    /// * `sample_count` - Number of samples to take (default: 256)
    pub fn from_curve(name: String, curve: &dyn Curve1D, sample_count: usize) -> Self {
        let (min_input, max_input) = curve.domain();
        let inv_range = 1.0 / (max_input - min_input);

        let mut samples = Vec::with_capacity(sample_count);

        for i in 0..sample_count {
            let t = i as f32 / (sample_count - 1) as f32;
            let x = min_input + t * (max_input - min_input);
            let y = curve.evaluate(x);
            samples.push(y);
        }

        Self {
            name,
            min_input,
            max_input,
            inv_range,
            samples,
        }
    }

    /// Create a lookup table with explicit parameters.
    ///
    /// # Arguments
    /// * `name` - Name of the table
    /// * `min_input` - Minimum input value
    /// * `max_input` - Maximum input value
    /// * `samples` - Precomputed sample values
    pub fn new(name: String, min_input: f32, max_input: f32, samples: Vec<f32>) -> Self {
        let inv_range = 1.0 / (max_input - min_input);
        Self {
            name,
            min_input,
            max_input,
            inv_range,
            samples,
        }
    }

    /// Get a value from the table with linear interpolation.
    ///
    /// # Arguments
    /// * `input` - Input value to look up
    ///
    /// # Returns
    /// Interpolated output value
    pub fn get_value(&self, input: f32) -> f32 {
        // Clamp to bounds
        if input <= self.min_input {
            return self.samples[0];
        }
        if input >= self.max_input {
            return self.samples[self.samples.len() - 1];
        }

        // Normalize input to [0, 1] and scale to sample indices
        let normalized = (input - self.min_input) * self.inv_range;
        let scaled = normalized * (self.samples.len() - 1) as f32;
        let index0 = scaled.floor() as usize;
        let index1 = (index0 + 1).min(self.samples.len() - 1);
        let lerp = scaled - index0 as f32;

        // Linear interpolation
        self.samples[index0] + lerp * (self.samples[index1] - self.samples[index0])
    }

    /// Get a value from the table without interpolation (nearest neighbor).
    ///
    /// This is faster but less accurate than `get_value`.
    ///
    /// # Arguments
    /// * `input` - Input value to look up
    ///
    /// # Returns
    /// Nearest sample value
    pub fn get_value_quick(&self, input: f32) -> f32 {
        // Clamp to bounds
        if input <= self.min_input {
            return self.samples[0];
        }
        if input >= self.max_input {
            return self.samples[self.samples.len() - 1];
        }

        // Find nearest sample
        let normalized = (input - self.min_input) * self.inv_range;
        let index = ((normalized * (self.samples.len() - 1) as f32).round() as usize)
            .min(self.samples.len() - 1);

        self.samples[index]
    }

    /// Get the name of the table.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the domain of the table.
    ///
    /// # Returns
    /// Tuple of (min_input, max_input)
    pub fn domain(&self) -> (f32, f32) {
        (self.min_input, self.max_input)
    }

    /// Get the number of samples in the table.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Get a reference to the raw sample data.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

/// Manager for lookup tables that handles loading and caching.
///
/// This provides a global registry of lookup tables that can be shared
/// across the application.
pub struct LookupTableManager {
    tables: Arc<Mutex<HashMap<String, Arc<LookupTable>>>>,
}

impl LookupTableManager {
    /// Create a new lookup table manager.
    pub fn new() -> Self {
        Self {
            tables: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a table to the manager.
    ///
    /// # Arguments
    /// * `table` - Table to add
    ///
    /// # Returns
    /// `true` if the table was added, `false` if a table with that name already exists
    pub fn add_table(&self, table: LookupTable) -> bool {
        let mut tables = self.tables.lock().unwrap();
        let name = table.name.clone();

        match tables.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(table));
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    /// Get a table by name.
    ///
    /// # Arguments
    /// * `name` - Name of the table to retrieve
    ///
    /// # Returns
    /// Reference-counted table, or None if not found
    pub fn get_table(&self, name: &str) -> Option<Arc<LookupTable>> {
        let tables = self.tables.lock().unwrap();
        tables.get(name).cloned()
    }

    /// Remove a table from the manager.
    ///
    /// # Arguments
    /// * `name` - Name of the table to remove
    ///
    /// # Returns
    /// `true` if the table was removed, `false` if it wasn't found
    pub fn remove_table(&self, name: &str) -> bool {
        let mut tables = self.tables.lock().unwrap();
        tables.remove(name).is_some()
    }

    /// Clear all tables from the manager.
    pub fn clear(&self) {
        let mut tables = self.tables.lock().unwrap();
        tables.clear();
    }

    /// Get the number of tables in the manager.
    pub fn table_count(&self) -> usize {
        let tables = self.tables.lock().unwrap();
        tables.len()
    }

    /// Get a list of all table names.
    pub fn table_names(&self) -> Vec<String> {
        let tables = self.tables.lock().unwrap();
        tables.keys().cloned().collect()
    }
}

impl Default for LookupTableManager {
    fn default() -> Self {
        let manager = Self::new();

        // Add a default linear table
        let default_curve = LinearCurve::new(1.0, 0.0, 0.0, 1.0);
        let default_table =
            LookupTable::from_curve("DefaultTable".to_string(), &default_curve, 256);
        manager.add_table(default_table);

        manager
    }
}

// Global table manager instance
lazy_static::lazy_static! {
    static ref GLOBAL_MANAGER: LookupTableManager = LookupTableManager::default();
}

/// Get the global lookup table manager.
pub fn global_table_manager() -> &'static LookupTableManager {
    &GLOBAL_MANAGER
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn assert_approx_eq(a: f32, b: f32, tolerance: f32) {
        assert!(
            (a - b).abs() < tolerance,
            "Expected {} ≈ {}, difference: {}",
            a,
            b,
            (a - b).abs()
        );
    }

    #[test]
    fn test_linear_curve() {
        let curve = LinearCurve::new(2.0, 1.0, 0.0, 10.0);

        assert_approx_eq(curve.evaluate(0.0), 1.0, 1e-6);
        assert_approx_eq(curve.evaluate(5.0), 11.0, 1e-6);
        assert_approx_eq(curve.evaluate(10.0), 21.0, 1e-6);

        assert_eq!(curve.domain(), (0.0, 10.0));
    }

    #[test]
    fn test_linear_curve_from_points() {
        let curve = LinearCurve::from_points(0.0, 1.0, 5.0, 11.0);

        assert_approx_eq(curve.evaluate(0.0), 1.0, 1e-6);
        assert_approx_eq(curve.evaluate(5.0), 11.0, 1e-6);
        assert_approx_eq(curve.evaluate(2.5), 6.0, 1e-6);
    }

    #[test]
    fn test_sine_curve() {
        let curve = SineCurve::new(1.0, 1.0, 0.0, 0.0, 0.0, 2.0 * PI);

        assert_approx_eq(curve.evaluate(0.0), 0.0, 1e-6);
        assert_approx_eq(curve.evaluate(PI / 2.0), 1.0, 1e-6);
        assert_approx_eq(curve.evaluate(PI), 0.0, 1e-6);
        assert_approx_eq(curve.evaluate(3.0 * PI / 2.0), -1.0, 1e-6);
    }

    #[test]
    fn test_lookup_table_creation() {
        let curve = LinearCurve::new(2.0, 1.0, 0.0, 10.0);
        let table = LookupTable::from_curve("test".to_string(), &curve, 11);

        assert_eq!(table.name(), "test");
        assert_eq!(table.domain(), (0.0, 10.0));
        assert_eq!(table.sample_count(), 11);

        // Check first and last samples
        assert_approx_eq(table.samples()[0], 1.0, 1e-6);
        assert_approx_eq(table.samples()[10], 21.0, 1e-6);
    }

    #[test]
    fn test_lookup_table_interpolation() {
        let curve = LinearCurve::new(1.0, 0.0, 0.0, 10.0);
        let table = LookupTable::from_curve("test".to_string(), &curve, 11);

        // Test exact sample points
        assert_approx_eq(table.get_value(0.0), 0.0, 1e-6);
        assert_approx_eq(table.get_value(10.0), 10.0, 1e-6);
        assert_approx_eq(table.get_value(5.0), 5.0, 1e-6);

        // Test interpolated values
        assert_approx_eq(table.get_value(2.5), 2.5, 0.1);
        assert_approx_eq(table.get_value(7.5), 7.5, 0.1);
    }

    #[test]
    fn test_lookup_table_clamping() {
        let curve = LinearCurve::new(1.0, 0.0, 0.0, 10.0);
        let table = LookupTable::from_curve("test".to_string(), &curve, 11);

        // Test values outside domain
        assert_approx_eq(table.get_value(-5.0), 0.0, 1e-6);
        assert_approx_eq(table.get_value(15.0), 10.0, 1e-6);
    }

    #[test]
    fn test_lookup_table_quick() {
        let curve = LinearCurve::new(1.0, 0.0, 0.0, 10.0);
        let table = LookupTable::from_curve("test".to_string(), &curve, 11);

        // Test quick lookup (nearest neighbor)
        let quick_value = table.get_value_quick(2.3);
        let interpolated_value = table.get_value(2.3);

        // Quick lookup should be less accurate but still reasonable
        assert!((quick_value - 2.3).abs() < 1.0);
        assert!((interpolated_value - 2.3).abs() < (quick_value - 2.3).abs());
    }

    #[test]
    fn test_lookup_table_manager() {
        let manager = LookupTableManager::new();

        let curve = LinearCurve::new(1.0, 0.0, 0.0, 1.0);
        let table = LookupTable::from_curve("test_table".to_string(), &curve, 100);

        // Add table
        assert!(manager.add_table(table));
        assert_eq!(manager.table_count(), 1);

        // Get table
        let retrieved = manager.get_table("test_table");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "test_table");

        // Try to add duplicate
        let duplicate = LookupTable::from_curve("test_table".to_string(), &curve, 50);
        assert!(!manager.add_table(duplicate));
        assert_eq!(manager.table_count(), 1);

        // Remove table
        assert!(manager.remove_table("test_table"));
        assert_eq!(manager.table_count(), 0);

        // Try to remove non-existent table
        assert!(!manager.remove_table("non_existent"));
    }

    #[test]
    fn test_global_table_manager() {
        let manager = global_table_manager();

        // Should have default table
        let default_table = manager.get_table("DefaultTable");
        assert!(default_table.is_some());

        // Test that it's actually a working table
        let table = default_table.unwrap();
        let value = table.get_value(0.5);
        assert!(value.is_finite());
    }

    #[test]
    fn test_sine_lookup_table_accuracy() {
        let curve = SineCurve::new(1.0, 1.0, 0.0, 0.0, 0.0, 2.0 * PI);
        let table = LookupTable::from_curve("sine".to_string(), &curve, 1000);

        // Test accuracy at various points
        let test_points = [
            0.0,
            PI / 4.0,
            PI / 2.0,
            3.0 * PI / 4.0,
            PI,
            5.0 * PI / 4.0,
            3.0 * PI / 2.0,
            7.0 * PI / 4.0,
        ];

        for &x in &test_points {
            let expected = curve.evaluate(x);
            let actual = table.get_value(x);
            assert_approx_eq(actual, expected, 0.01);
        }
    }

    #[test]
    fn test_table_manager_names() {
        let manager = LookupTableManager::new();

        let curve = LinearCurve::new(1.0, 0.0, 0.0, 1.0);
        let table1 = LookupTable::from_curve("table1".to_string(), &curve, 100);
        let table2 = LookupTable::from_curve("table2".to_string(), &curve, 100);

        manager.add_table(table1);
        manager.add_table(table2);

        let names = manager.table_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"table1".to_string()));
        assert!(names.contains(&"table2".to_string()));

        manager.clear();
        assert_eq!(manager.table_count(), 0);
        assert!(manager.table_names().is_empty());
    }
}
