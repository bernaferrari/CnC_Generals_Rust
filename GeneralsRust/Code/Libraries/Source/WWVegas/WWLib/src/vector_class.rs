//! Vector class implementation for WWLib
//!
//! This module provides Rust implementations of the Command & Conquer Generals
//! WWLib vector classes. It includes both `VectorClass` (fixed-size) and
//! `DynamicVectorClass` (resizable) containers that maintain API compatibility
//! with the original C++ implementation while leveraging Rust's memory safety.
//!
//! # Examples
//!
//! ```rust
//! use wwlib_rust::vector_class::{VectorClass, DynamicVectorClass};
//!
//! // Create a fixed-size vector
//! let mut vec: VectorClass<i32> = VectorClass::new(5, None);
//! vec[0] = 42;
//!
//! // Create a dynamic vector
//! let mut dynamic_vec: DynamicVectorClass<String> = DynamicVectorClass::new(0, None);
//! dynamic_vec.add("Hello".to_string());
//! dynamic_vec.add("World".to_string());
//! ```

use std::fmt;
use std::ops::{Index, IndexMut};
use std::slice;

/// Error types for vector operations
#[derive(Debug, Clone, PartialEq)]
pub enum VectorError {
    /// Index out of bounds
    IndexOutOfBounds { index: usize, len: usize },
    /// Memory allocation failed
    AllocationFailed,
    /// Operation not supported
    OperationNotSupported,
    /// Vector is in invalid state
    InvalidState,
}

impl fmt::Display for VectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VectorError::IndexOutOfBounds { index, len } => {
                write!(
                    f,
                    "Index {} out of bounds for vector of length {}",
                    index, len
                )
            }
            VectorError::AllocationFailed => write!(f, "Memory allocation failed"),
            VectorError::OperationNotSupported => write!(f, "Operation not supported"),
            VectorError::InvalidState => write!(f, "Vector is in invalid state"),
        }
    }
}

impl std::error::Error for VectorError {}

/// Result type for vector operations
pub type VectorResult<T> = Result<T, VectorError>;

/// A fixed-size vector class that mimics the C++ VectorClass template.
///
/// This class provides a container for a fixed number of elements of type T.
/// Unlike Rust's Vec, the size is fixed at creation time and cannot be changed
/// dynamically (except through explicit resize operations).
///
/// # Type Parameters
///
/// * `T` - The type of elements stored in the vector. Must implement Clone.
///
/// # Examples
///
/// ```rust
/// let mut vec = VectorClass::new(3, None);
/// vec[0] = 42;
/// vec[1] = 24;
/// vec[2] = 13;
/// assert_eq!(vec.length(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct VectorClass<T: Clone> {
    /// The underlying storage for vector elements
    vector: Vec<T>,
    /// Maximum number of elements allowed in this vector
    vector_max: usize,
    /// Whether the vector is in a valid state
    is_valid: bool,
    /// Whether the vector data is allocated by this instance
    is_allocated: bool,
}

impl<T: Clone> VectorClass<T> {
    /// Creates a new VectorClass with the specified size.
    ///
    /// # Arguments
    ///
    /// * `size` - The number of elements to initialize this vector to
    /// * `array` - Optional slice to copy initial data from
    ///
    /// # Examples
    ///
    /// ```rust
    /// let vec: VectorClass<i32> = VectorClass::new(5, None);
    /// assert_eq!(vec.length(), 5);
    /// ```
    pub fn new(size: usize, array: Option<&[T]>) -> Self
    where
        T: Default,
    {
        let mut vector = Vec::with_capacity(size);

        if let Some(source_array) = array {
            let copy_count = size.min(source_array.len());
            vector.extend_from_slice(&source_array[..copy_count]);
            // Fill remaining slots with default values if needed
            while vector.len() < size {
                vector.push(T::default());
            }
        } else if size > 0 {
            // Initialize with default values
            vector.resize(size, T::default());
        }

        VectorClass {
            vector,
            vector_max: size,
            is_valid: true,
            is_allocated: true,
        }
    }

    /// Creates a new empty VectorClass (used for implementing NoInitClass equivalent).
    pub fn new_uninitialized() -> Self {
        VectorClass {
            vector: Vec::new(),
            vector_max: 0,
            is_valid: true,
            is_allocated: false,
        }
    }

    /// Returns the maximum number of elements in this vector.
    pub fn length(&self) -> usize {
        self.vector_max
    }

    /// Clears and deallocates the vector.
    pub fn clear(&mut self) {
        self.vector.clear();
        self.vector.shrink_to_fit();
        self.vector_max = 0;
        self.is_allocated = false;
    }

    /// Changes the size of the vector.
    ///
    /// # Arguments
    ///
    /// * `new_size` - The desired size of the vector
    /// * `array` - Optional slice to copy data from
    ///
    /// # Returns
    ///
    /// `true` if the resize was successful, `false` otherwise
    pub fn resize(&mut self, new_size: usize, array: Option<&[T]>) -> bool
    where
        T: Default,
    {
        if new_size == 0 {
            self.clear();
            return true;
        }

        // Create new vector with desired capacity
        let mut new_vector = Vec::with_capacity(new_size);

        if let Some(source_array) = array {
            let copy_count = new_size.min(source_array.len());
            new_vector.extend_from_slice(&source_array[..copy_count]);
            while new_vector.len() < new_size {
                new_vector.push(T::default());
            }
        } else {
            // Copy existing elements
            let copy_count = new_size.min(self.vector.len());
            if copy_count > 0 {
                new_vector.extend_from_slice(&self.vector[..copy_count]);
            }
            // Fill remaining with default values
            while new_vector.len() < new_size {
                new_vector.push(T::default());
            }
        }

        self.vector = new_vector;
        self.vector_max = new_size;
        self.is_allocated = true;
        true
    }

    /// Finds the index of a value in the vector.
    ///
    /// # Arguments
    ///
    /// * `object` - Reference to the object to find
    ///
    /// # Returns
    ///
    /// The index of the object, or -1 if not found (maintains C++ API compatibility)
    pub fn id_by_value(&self, object: &T) -> i32
    where
        T: PartialEq,
    {
        if !self.is_valid {
            return 0;
        }

        for (index, item) in self.vector.iter().enumerate() {
            if item == object {
                return index as i32;
            }
        }
        -1
    }

    /// Finds the index of an element by its memory address (pointer).
    ///
    /// # Arguments
    ///
    /// * `ptr` - Pointer to an element within this vector
    ///
    /// # Returns
    ///
    /// The index of the element, or an invalid index if the pointer is not within this vector
    pub fn id_by_ptr(&self, ptr: *const T) -> i32 {
        if !self.is_valid || self.vector.is_empty() {
            return 0;
        }

        let base_ptr = self.vector.as_ptr();
        let offset = unsafe { ptr.offset_from(base_ptr) };

        if offset >= 0 && (offset as usize) < self.vector.len() {
            offset as i32
        } else {
            -1
        }
    }

    /// Returns whether the vector is in a valid state.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Returns whether the vector data is allocated by this instance.
    pub fn is_allocated(&self) -> bool {
        self.is_allocated
    }

    /// Get a slice view of the vector data.
    pub fn as_slice(&self) -> &[T] {
        &self.vector
    }

    /// Get a mutable slice view of the vector data.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.vector
    }
}

impl<T: Clone + PartialEq> PartialEq for VectorClass<T> {
    /// Equality operator for vector objects.
    fn eq(&self, other: &Self) -> bool {
        if self.vector_max != other.vector_max {
            return false;
        }

        for i in 0..self.vector_max {
            if self.vector.get(i) != other.vector.get(i) {
                return false;
            }
        }

        true
    }
}

impl<T: Clone> Index<usize> for VectorClass<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(
            index < self.vector_max,
            "Index {} out of bounds for vector of length {}",
            index,
            self.vector_max
        );
        &self.vector[index]
    }
}

impl<T: Clone> IndexMut<usize> for VectorClass<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(
            index < self.vector_max,
            "Index {} out of bounds for vector of length {}",
            index,
            self.vector_max
        );
        &mut self.vector[index]
    }
}

// Also support i32 indexing for C++ compatibility
impl<T: Clone> Index<i32> for VectorClass<T> {
    type Output = T;

    fn index(&self, index: i32) -> &Self::Output {
        let index = index as usize;
        assert!(
            index < self.vector_max,
            "Index {} out of bounds for vector of length {}",
            index,
            self.vector_max
        );
        &self.vector[index]
    }
}

impl<T: Clone> IndexMut<i32> for VectorClass<T> {
    fn index_mut(&mut self, index: i32) -> &mut Self::Output {
        let index = index as usize;
        assert!(
            index < self.vector_max,
            "Index {} out of bounds for vector of length {}",
            index,
            self.vector_max
        );
        &mut self.vector[index]
    }
}

/// A dynamic vector class that extends VectorClass with add/remove functionality.
///
/// This class provides a resizable container similar to std::vector in C++.
/// Objects are packed to the beginning of the array, and the vector can grow
/// automatically when adding elements.
///
/// # Type Parameters
///
/// * `T` - The type of elements stored in the vector. Must implement Clone.
///
/// # Examples
///
/// ```rust
/// let mut vec = DynamicVectorClass::new(0, None);
/// vec.add(String::from("Hello"));
/// vec.add(String::from("World"));
/// assert_eq!(vec.count(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct DynamicVectorClass<T: Clone> {
    /// Base vector functionality
    base: VectorClass<T>,
    /// Number of active objects in the vector
    active_count: usize,
    /// Growth step for automatic resizing
    growth_step: usize,
}

impl<T: Clone> DynamicVectorClass<T> {
    /// Creates a new DynamicVectorClass.
    ///
    /// # Arguments
    ///
    /// * `size` - Initial maximum size of the vector
    /// * `array` - Optional slice to copy initial data from
    ///
    /// # Examples
    ///
    /// ```rust
    /// let vec: DynamicVectorClass<i32> = DynamicVectorClass::new(10, None);
    /// assert_eq!(vec.length(), 10);
    /// assert_eq!(vec.count(), 0);
    /// ```
    pub fn new(size: usize, array: Option<&[T]>) -> Self
    where
        T: Default,
    {
        DynamicVectorClass {
            base: VectorClass::new(size, array),
            active_count: 0,
            growth_step: 10,
        }
    }

    /// Creates a new empty DynamicVectorClass.
    pub fn new_uninitialized() -> Self {
        DynamicVectorClass {
            base: VectorClass::new_uninitialized(),
            active_count: 0,
            growth_step: 10,
        }
    }

    /// Returns the maximum capacity of the vector.
    pub fn length(&self) -> usize {
        self.base.length()
    }

    /// Returns the number of active elements in the vector.
    pub fn count(&self) -> usize {
        self.active_count
    }

    /// Sets the growth step for automatic resizing.
    ///
    /// # Arguments
    ///
    /// * `step` - Number of elements to grow by when resizing
    ///
    /// # Returns
    ///
    /// The previous growth step value
    pub fn set_growth_step(&mut self, step: usize) -> usize {
        let old_step = self.growth_step;
        self.growth_step = step;
        old_step
    }

    /// Gets the current growth step.
    pub fn growth_step(&self) -> usize {
        self.growth_step
    }

    /// Resets the active count to zero without deallocating memory.
    pub fn reset_active(&mut self) {
        self.active_count = 0;
    }

    /// Sets the active count to the specified value.
    ///
    /// # Arguments
    ///
    /// * `count` - New active count (must not exceed vector length)
    pub fn set_active(&mut self, count: usize) {
        self.active_count = count.min(self.base.length());
    }

    /// Clears the vector and deallocates memory.
    pub fn clear(&mut self) {
        self.active_count = 0;
        self.base.clear();
    }

    /// Resizes the vector to a new maximum size.
    ///
    /// # Arguments
    ///
    /// * `new_size` - The desired maximum size
    /// * `array` - Optional slice to copy data from
    ///
    /// # Returns
    ///
    /// `true` if resize was successful, `false` otherwise
    pub fn resize(&mut self, new_size: usize, array: Option<&[T]>) -> bool
    where
        T: Default,
    {
        if self.base.resize(new_size, array) {
            if self.base.length() < self.active_count {
                self.active_count = self.base.length();
            }
            true
        } else {
            false
        }
    }

    /// Adds an element to the end of the vector.
    ///
    /// The vector will automatically grow if there's insufficient space and
    /// growth is allowed (growth_step > 0).
    ///
    /// # Arguments
    ///
    /// * `object` - The object to add to the vector
    ///
    /// # Returns
    ///
    /// `true` if the object was added successfully, `false` otherwise
    pub fn add(&mut self, object: T) -> bool
    where
        T: Default,
    {
        if self.active_count >= self.length() {
            if (self.base.is_allocated() || self.base.length() == 0) && self.growth_step > 0 {
                if !self.resize(self.length() + self.growth_step, None) {
                    return false;
                }
            } else {
                return false;
            }
        }

        self.base[self.active_count] = object;
        self.active_count += 1;
        true
    }

    /// Adds an element to the beginning of the vector.
    ///
    /// All existing elements are shifted one position to the right.
    ///
    /// # Arguments
    ///
    /// * `object` - The object to add to the head of the vector
    ///
    /// # Returns
    ///
    /// `true` if the object was added successfully, `false` otherwise
    pub fn add_head(&mut self, object: T) -> bool
    where
        T: Default,
    {
        if self.active_count >= self.length() {
            if (self.base.is_allocated() || self.base.length() == 0) && self.growth_step > 0 {
                if !self.resize(self.length() + self.growth_step, None) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Shift existing elements to the right
        if self.active_count > 0 {
            for i in (0..self.active_count).rev() {
                self.base[i + 1] = self.base[i].clone();
            }
        }

        self.base[0] = object;
        self.active_count += 1;
        true
    }

    /// Inserts an element at the specified index.
    ///
    /// All elements at and after the insertion point are shifted one position to the right.
    ///
    /// # Arguments
    ///
    /// * `index` - Index where to insert the object
    /// * `object` - The object to insert
    ///
    /// # Returns
    ///
    /// `true` if the object was inserted successfully, `false` otherwise
    pub fn insert(&mut self, index: usize, object: T) -> bool
    where
        T: Default,
    {
        if index > self.active_count {
            return false;
        }

        if self.active_count >= self.length() {
            if (self.base.is_allocated() || self.base.length() == 0) && self.growth_step > 0 {
                if !self.resize(self.length() + self.growth_step, None) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Shift elements to the right
        if index < self.active_count {
            for i in (index..self.active_count).rev() {
                self.base[i + 1] = self.base[i].clone();
            }
        }

        self.base[index] = object;
        self.active_count += 1;
        true
    }

    /// Removes the first occurrence of the specified object from the vector.
    ///
    /// # Arguments
    ///
    /// * `object` - Reference to the object to remove
    ///
    /// # Returns
    ///
    /// `true` if the object was found and removed, `false` otherwise
    pub fn delete_by_value(&mut self, object: &T) -> bool
    where
        T: PartialEq,
    {
        let id = self.id_by_value(object);
        if id != -1 {
            self.delete_by_index(id as usize)
        } else {
            false
        }
    }

    /// Removes the element at the specified index.
    ///
    /// All elements after the removed element are shifted one position to the left.
    ///
    /// # Arguments
    ///
    /// * `index` - Index of the element to remove
    ///
    /// # Returns
    ///
    /// `true` if the element was removed successfully, `false` if index was out of bounds
    pub fn delete_by_index(&mut self, index: usize) -> bool {
        if index < self.active_count {
            self.active_count -= 1;

            // Shift elements to the left
            for i in index..self.active_count {
                self.base[i] = self.base[i + 1].clone();
            }

            true
        } else {
            false
        }
    }

    /// Legacy method name for compatibility with C++ API.
    pub fn delete(&mut self, index: usize) -> bool {
        self.delete_by_index(index)
    }

    /// Explicit delete by index method for C++ API compatibility.
    pub fn delete_index(&mut self, index: usize) -> bool {
        self.delete_by_index(index)
    }

    /// Deletes all objects in the vector.
    ///
    /// This preserves the allocated capacity but resets the active count.
    pub fn delete_all(&mut self)
    where
        T: Default,
    {
        let len = self.base.vector_max;
        self.clear();
        let _ = self.resize(len, None);
    }

    /// Finds the index of a value in the active portion of the vector.
    ///
    /// Unlike the base class version, this only searches the active elements.
    ///
    /// # Arguments
    ///
    /// * `object` - Reference to the object to find
    ///
    /// # Returns
    ///
    /// The index of the object, or -1 if not found
    pub fn id_by_value(&self, object: &T) -> i32
    where
        T: PartialEq,
    {
        for index in 0..self.active_count {
            if &self.base[index] == object {
                return index as i32;
            }
        }
        -1
    }

    /// Finds the index of an element by its memory address (pointer).
    ///
    /// # Arguments
    ///
    /// * `ptr` - Pointer to an element within this vector
    ///
    /// # Returns
    ///
    /// The index of the element, or -1 if the pointer is not within this vector
    pub fn id_by_ptr(&self, ptr: *const T) -> i32 {
        self.base.id_by_ptr(ptr)
    }

    /// Adds an uninitialized element to the vector and returns a mutable reference to it.
    ///
    /// This is equivalent to the C++ `Uninitialized_Add` method. The caller is responsible
    /// for properly initializing the returned reference before using the vector further.
    ///
    /// # Returns
    ///
    /// A mutable reference to the new element, or None if the operation failed
    pub fn uninitialized_add(&mut self) -> Option<&mut T>
    where
        T: Default,
    {
        if self.active_count >= self.length() {
            if self.growth_step > 0 {
                if !self.resize(self.length() + self.growth_step, None) {
                    return None;
                }
            } else {
                return None;
            }
        }

        let index = self.active_count;
        self.active_count += 1;
        Some(&mut self.base[index])
    }

    /// Get an iterator over the active elements.
    pub fn iter(&self) -> slice::Iter<T> {
        self.base.vector[..self.active_count].iter()
    }

    /// Get a mutable iterator over the active elements.
    pub fn iter_mut(&mut self) -> slice::IterMut<T> {
        self.base.vector[..self.active_count].iter_mut()
    }

    /// Get a slice view of the active elements.
    pub fn as_slice(&self) -> &[T] {
        &self.base.vector[..self.active_count]
    }

    /// Get a mutable slice view of the active elements.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.base.vector[..self.active_count]
    }
}

// Implement PartialEq for DynamicVectorClass
impl<T: Clone + PartialEq> PartialEq for DynamicVectorClass<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.active_count != other.active_count {
            return false;
        }

        for i in 0..self.active_count {
            if self.base[i] != other.base[i] {
                return false;
            }
        }

        true
    }
}

// Implement Index and IndexMut for DynamicVectorClass
impl<T: Clone> Index<usize> for DynamicVectorClass<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(
            index < self.active_count,
            "Index {} out of bounds for dynamic vector with {} active elements",
            index,
            self.active_count
        );
        &self.base[index]
    }
}

impl<T: Clone> IndexMut<usize> for DynamicVectorClass<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(
            index < self.active_count,
            "Index {} out of bounds for dynamic vector with {} active elements",
            index,
            self.active_count
        );
        &mut self.base[index]
    }
}

impl<T: Clone> Index<i32> for DynamicVectorClass<T> {
    type Output = T;

    fn index(&self, index: i32) -> &Self::Output {
        let index = index as usize;
        assert!(
            index < self.active_count,
            "Index {} out of bounds for dynamic vector with {} active elements",
            index,
            self.active_count
        );
        &self.base[index]
    }
}

impl<T: Clone> IndexMut<i32> for DynamicVectorClass<T> {
    fn index_mut(&mut self, index: i32) -> &mut Self::Output {
        let index = index as usize;
        assert!(
            index < self.active_count,
            "Index {} out of bounds for dynamic vector with {} active elements",
            index,
            self.active_count
        );
        &mut self.base[index]
    }
}

// Implement assignment operator equivalent
impl<T: Clone> DynamicVectorClass<T> {
    /// Assignment operation equivalent to C++ operator=
    pub fn assign_from(&mut self, other: &DynamicVectorClass<T>)
    where
        T: Default,
    {
        self.base = other.base.clone();
        self.active_count = other.active_count;
        self.growth_step = other.growth_step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_class_creation() {
        let vec: VectorClass<i32> = VectorClass::new(5, None);
        assert_eq!(vec.length(), 5);
        assert!(vec.is_valid());
        assert!(vec.is_allocated());
    }

    #[test]
    fn test_vector_class_with_initial_data() {
        let data = [1, 2, 3, 4, 5];
        let vec = VectorClass::new(5, Some(&data));
        assert_eq!(vec.length(), 5);
        for i in 0..5 {
            assert_eq!(vec[i], data[i]);
        }
    }

    #[test]
    fn test_vector_class_indexing() {
        let mut vec: VectorClass<i32> = VectorClass::new(3, None);
        vec[0] = 10;
        vec[1] = 20;
        vec[2] = 30;

        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);
    }

    #[test]
    fn test_vector_class_resize() {
        let mut vec: VectorClass<i32> = VectorClass::new(3, None);
        vec[0] = 10;
        vec[1] = 20;
        vec[2] = 30;

        assert!(vec.resize(5, None));
        assert_eq!(vec.length(), 5);
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);
    }

    #[test]
    fn test_vector_class_equality() {
        let data = [1, 2, 3];
        let vec1 = VectorClass::new(3, Some(&data));
        let vec2 = VectorClass::new(3, Some(&data));
        let vec3 = VectorClass::new(3, None);

        assert_eq!(vec1, vec2);
        assert_ne!(vec1, vec3);
    }

    #[test]
    fn test_vector_class_id_by_value() {
        let data = [10, 20, 30];
        let vec = VectorClass::new(3, Some(&data));

        assert_eq!(vec.id_by_value(&20), 1);
        assert_eq!(vec.id_by_value(&40), -1);
    }

    #[test]
    fn test_dynamic_vector_class_creation() {
        let vec: DynamicVectorClass<i32> = DynamicVectorClass::new(10, None);
        assert_eq!(vec.length(), 10);
        assert_eq!(vec.count(), 0);
        assert_eq!(vec.growth_step(), 10);
    }

    #[test]
    fn test_dynamic_vector_add() {
        let mut vec = DynamicVectorClass::new(0, None);

        assert!(vec.add(10));
        assert!(vec.add(20));
        assert!(vec.add(30));

        assert_eq!(vec.count(), 3);
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);
    }

    #[test]
    fn test_dynamic_vector_add_head() {
        let mut vec = DynamicVectorClass::new(0, None);

        assert!(vec.add(20));
        assert!(vec.add(30));
        assert!(vec.add_head(10));

        assert_eq!(vec.count(), 3);
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);
    }

    #[test]
    fn test_dynamic_vector_insert() {
        let mut vec = DynamicVectorClass::new(0, None);

        assert!(vec.add(10));
        assert!(vec.add(30));
        assert!(vec.insert(1, 20));

        assert_eq!(vec.count(), 3);
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);
    }

    #[test]
    fn test_dynamic_vector_delete() {
        let mut vec = DynamicVectorClass::new(0, None);

        vec.add(10);
        vec.add(20);
        vec.add(30);

        assert!(vec.delete_by_index(1));
        assert_eq!(vec.count(), 2);
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 30);
    }

    #[test]
    fn test_dynamic_vector_delete_by_value() {
        let mut vec = DynamicVectorClass::new(0, None);

        vec.add(10);
        vec.add(20);
        vec.add(30);

        assert!(vec.delete_by_value(&20));
        assert_eq!(vec.count(), 2);
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 30);

        assert!(!vec.delete_by_value(&40));
    }

    #[test]
    fn test_dynamic_vector_growth() {
        let mut vec = DynamicVectorClass::new(2, None);
        vec.set_growth_step(3);

        assert!(vec.add(1));
        assert!(vec.add(2));
        assert!(vec.add(3)); // This should trigger growth

        assert_eq!(vec.count(), 3);
        assert!(vec.length() >= 3);
        assert_eq!(vec[0], 1);
        assert_eq!(vec[1], 2);
        assert_eq!(vec[2], 3);
    }

    #[test]
    fn test_dynamic_vector_id() {
        let mut vec = DynamicVectorClass::new(0, None);

        vec.add(10);
        vec.add(20);
        vec.add(30);

        assert_eq!(vec.id_by_value(&20), 1);
        assert_eq!(vec.id_by_value(&40), -1);
    }

    #[test]
    fn test_dynamic_vector_uninitialized_add() {
        let mut vec: DynamicVectorClass<i32> = DynamicVectorClass::new(0, None);

        if let Some(slot) = vec.uninitialized_add() {
            *slot = 42;
            assert_eq!(vec.count(), 1);
            assert_eq!(vec[0], 42);
        } else {
            panic!("uninitialized_add should have succeeded");
        }
    }

    #[test]
    fn test_dynamic_vector_iterators() {
        let mut vec = DynamicVectorClass::new(0, None);
        vec.add(1);
        vec.add(2);
        vec.add(3);

        let collected: Vec<_> = vec.iter().cloned().collect();
        assert_eq!(collected, vec![1, 2, 3]);

        for item in vec.iter_mut() {
            *item *= 2;
        }

        let collected: Vec<_> = vec.iter().cloned().collect();
        assert_eq!(collected, vec![2, 4, 6]);
    }

    #[test]
    fn test_string_vector() {
        let mut vec = DynamicVectorClass::new(0, None);

        assert!(vec.add("Hello".to_string()));
        assert!(vec.add("World".to_string()));

        assert_eq!(vec.count(), 2);
        assert_eq!(vec[0], "Hello");
        assert_eq!(vec[1], "World");

        assert_eq!(vec.id_by_value(&"World".to_string()), 1);
        assert!(vec.delete_by_value(&"Hello".to_string()));
        assert_eq!(vec.count(), 1);
        assert_eq!(vec[0], "World");
    }
}
