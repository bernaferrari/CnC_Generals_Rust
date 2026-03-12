//! Audio List Implementation
//! 
//! This module provides a doubly-linked list implementation with priority-based ordering,
//! converted from the original C++ implementation in AUD_List.cpp.
//! 
//! The list supports:
//! - Priority-based insertion (ascending and descending)
//! - Safe iteration with Rust iterators
//! - Memory safety through Rust's ownership system
//! - Comprehensive error handling
//! 
//! # Examples
//! 
//! ```rust
//! use aud_list::{AudList, ListItem, Priority};
//! 
//! // Create a new list
//! let mut list = AudList::new();
//! 
//! // Add items with different priorities
//! list.add_node_ascending(ListItem::new(10, "High priority"));
//! list.add_node_ascending(ListItem::new(5, "Medium priority"));
//! list.add_node_ascending(ListItem::new(1, "Low priority"));
//! 
//! // Iterate through items (will be in priority order)
//! for item in &list {
//!     println!("Priority: {}, Data: {:?}", item.priority(), item.data());
//! }
//! ```

use std::collections::VecDeque;
use std::fmt;
use std::ptr;

/// Priority type for list items (equivalent to C++ Priority type)
pub type Priority = i32;

/// Priority constants from the original C++ code
pub const LOWEST_PRIORITY: Priority = i32::MIN;
pub const HIGHEST_PRIORITY: Priority = i32::MAX;
pub const NORMAL_PRIORITY: Priority = 0;

/// Errors that can occur during list operations
#[derive(Debug, Clone, PartialEq)]
pub enum ListError {
    /// Attempted to access an item at an invalid index
    IndexOutOfBounds(usize),
    /// Attempted to operate on an empty list when items were expected
    EmptyList,
    /// Item not found in the list
    ItemNotFound,
    /// Invalid priority value
    InvalidPriority,
}

impl fmt::Display for ListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListError::IndexOutOfBounds(index) => write!(f, "Index {} out of bounds", index),
            ListError::EmptyList => write!(f, "Operation on empty list"),
            ListError::ItemNotFound => write!(f, "Item not found in list"),
            ListError::InvalidPriority => write!(f, "Invalid priority value"),
        }
    }
}

impl std::error::Error for ListError {}

/// A list item containing data and priority information
/// 
/// This is equivalent to the C++ ListNode structure but with safe Rust data handling.
#[derive(Debug, Clone)]
pub struct ListItem<T> {
    data: T,
    priority: Priority,
}

impl<T> ListItem<T> {
    /// Creates a new list item with the specified priority and data
    /// 
    /// # Arguments
    /// 
    /// * `priority` - The priority for ordering in the list
    /// * `data` - The data to store in the item
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let item = ListItem::new(10, "Important data");
    /// assert_eq!(item.priority(), 10);
    /// ```
    pub fn new(priority: Priority, data: T) -> Self {
        Self { data, priority }
    }

    /// Returns the priority of this item
    pub fn priority(&self) -> Priority {
        self.priority
    }

    /// Sets the priority of this item
    pub fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
    }

    /// Returns a reference to the data
    pub fn data(&self) -> &T {
        &self.data
    }

    /// Returns a mutable reference to the data
    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// Consumes the item and returns the data
    pub fn into_data(self) -> T {
        self.data
    }
}

/// A doubly-linked priority list implementation
/// 
/// This provides the same functionality as the original C++ list implementation
/// but with Rust's memory safety guarantees. The list maintains items in priority order
/// and supports various insertion and access patterns.
#[derive(Debug, Clone)]
pub struct AudList<T> {
    items: VecDeque<ListItem<T>>,
}

impl<T> Default for AudList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AudList<T> {
    /// Creates a new empty list
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let list: AudList<String> = AudList::new();
    /// assert!(list.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }

    /// Initializes the list (equivalent to C++ ListInit)
    /// 
    /// This is essentially a no-op in Rust since the list is properly initialized
    /// when created, but provided for API compatibility.
    pub fn init(&mut self) {
        self.items.clear();
    }

    /// Adds a node to the list in ascending priority order (equivalent to C++ ListAddNodeSortAscending)
    /// 
    /// Returns the index where the item was inserted.
    /// 
    /// # Arguments
    /// 
    /// * `item` - The item to add to the list
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let mut list = AudList::new();
    /// let index = list.add_node_ascending(ListItem::new(5, "data"));
    /// assert_eq!(index, 0);
    /// ```
    pub fn add_node_ascending(&mut self, item: ListItem<T>) -> usize {
        let priority = item.priority();
        
        for (index, existing_item) in self.items.iter().enumerate() {
            if priority <= existing_item.priority() {
                self.items.insert(index, item);
                return index;
            }
        }
        
        // Add at the end if no suitable position found
        let index = self.items.len();
        self.items.push_back(item);
        index
    }

    /// Adds a node to the list in descending priority order (equivalent to C++ ListAddNode)
    /// 
    /// # Arguments
    /// 
    /// * `item` - The item to add to the list
    pub fn add_node(&mut self, item: ListItem<T>) {
        let priority = item.priority();
        
        for (index, existing_item) in self.items.iter().enumerate() {
            if existing_item.priority() <= priority {
                self.items.insert(index, item);
                return;
            }
        }
        
        // Add at the end if no suitable position found
        self.items.push_back(item);
    }

    /// Adds a node after items with the same priority (equivalent to C++ ListAddNodeAfter)
    /// 
    /// # Arguments
    /// 
    /// * `item` - The item to add to the list
    pub fn add_node_after(&mut self, item: ListItem<T>) {
        let priority = item.priority();
        
        for (index, existing_item) in self.items.iter().enumerate() {
            if existing_item.priority() < priority {
                self.items.insert(index, item);
                return;
            }
        }
        
        // Add at the end if no suitable position found
        self.items.push_back(item);
    }

    /// Merges another list into this one (equivalent to C++ ListMerge)
    /// 
    /// The other list will be empty after this operation.
    /// 
    /// # Arguments
    /// 
    /// * `other` - The list to merge into this one
    pub fn merge(&mut self, other: &mut Self) {
        // Append all items from other list
        self.items.append(&mut other.items);
        
        // Re-sort to maintain priority order
        self.items.make_contiguous().sort_by_key(|item| item.priority());
    }

    /// Returns the number of items in the list (equivalent to C++ ListCountItems)
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Returns the number of items in the list (alias for count)
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if the list is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns a reference to the first item (equivalent to C++ ListFirstItem)
    /// 
    /// # Returns
    /// 
    /// * `Some(&ListItem<T>)` - Reference to the first item
    /// * `None` - If the list is empty
    pub fn first_item(&self) -> Option<&ListItem<T>> {
        self.items.front()
    }

    /// Returns a mutable reference to the first item
    pub fn first_item_mut(&mut self) -> Option<&mut ListItem<T>> {
        self.items.front_mut()
    }

    /// Returns a reference to the last item
    pub fn last_item(&self) -> Option<&ListItem<T>> {
        self.items.back()
    }

    /// Returns a mutable reference to the last item
    pub fn last_item_mut(&mut self) -> Option<&mut ListItem<T>> {
        self.items.back_mut()
    }

    /// Returns a reference to the item at the specified index (equivalent to C++ ListGetItem)
    /// 
    /// # Arguments
    /// 
    /// * `index` - The zero-based index of the item to retrieve
    /// 
    /// # Returns
    /// 
    /// * `Ok(&ListItem<T>)` - Reference to the item at the index
    /// * `Err(ListError)` - If the index is out of bounds
    pub fn get_item(&self, index: usize) -> Result<&ListItem<T>, ListError> {
        self.items.get(index).ok_or(ListError::IndexOutOfBounds(index))
    }

    /// Returns a mutable reference to the item at the specified index
    pub fn get_item_mut(&mut self, index: usize) -> Result<&mut ListItem<T>, ListError> {
        self.items.get_mut(index).ok_or(ListError::IndexOutOfBounds(index))
    }

    /// Removes the item at the specified index
    /// 
    /// # Arguments
    /// 
    /// * `index` - The zero-based index of the item to remove
    /// 
    /// # Returns
    /// 
    /// * `Ok(ListItem<T>)` - The removed item
    /// * `Err(ListError)` - If the index is out of bounds
    pub fn remove_at(&mut self, index: usize) -> Result<ListItem<T>, ListError> {
        if index >= self.items.len() {
            return Err(ListError::IndexOutOfBounds(index));
        }
        Ok(self.items.remove(index).unwrap())
    }

    /// Removes the first item from the list
    /// 
    /// # Returns
    /// 
    /// * `Ok(ListItem<T>)` - The removed item
    /// * `Err(ListError)` - If the list is empty
    pub fn remove_first(&mut self) -> Result<ListItem<T>, ListError> {
        self.items.pop_front().ok_or(ListError::EmptyList)
    }

    /// Removes the last item from the list
    /// 
    /// # Returns
    /// 
    /// * `Ok(ListItem<T>)` - The removed item
    /// * `Err(ListError)` - If the list is empty
    pub fn remove_last(&mut self) -> Result<ListItem<T>, ListError> {
        self.items.pop_back().ok_or(ListError::EmptyList)
    }

    /// Clears all items from the list
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Returns an iterator over the items in the list
    pub fn iter(&self) -> std::collections::vec_deque::Iter<ListItem<T>> {
        self.items.iter()
    }

    /// Returns a mutable iterator over the items in the list
    pub fn iter_mut(&mut self) -> std::collections::vec_deque::IterMut<ListItem<T>> {
        self.items.iter_mut()
    }
}

impl<T: PartialEq> AudList<T> {
    /// Finds the first item with the specified data
    /// 
    /// # Arguments
    /// 
    /// * `data` - The data to search for
    /// 
    /// # Returns
    /// 
    /// * `Some(usize)` - The index of the first matching item
    /// * `None` - If no matching item is found
    pub fn find(&self, data: &T) -> Option<usize> {
        self.items.iter().position(|item| item.data() == data)
    }

    /// Removes the first item with the specified data
    /// 
    /// # Arguments
    /// 
    /// * `data` - The data to search for and remove
    /// 
    /// # Returns
    /// 
    /// * `Ok(ListItem<T>)` - The removed item
    /// * `Err(ListError)` - If no matching item is found
    pub fn remove_by_data(&mut self, data: &T) -> Result<ListItem<T>, ListError> {
        if let Some(index) = self.find(data) {
            self.remove_at(index)
        } else {
            Err(ListError::ItemNotFound)
        }
    }
}

// Iterator implementations for the list
impl<T> IntoIterator for AudList<T> {
    type Item = ListItem<T>;
    type IntoIter = std::collections::vec_deque::IntoIter<ListItem<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a AudList<T> {
    type Item = &'a ListItem<T>;
    type IntoIter = std::collections::vec_deque::Iter<'a, ListItem<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut AudList<T> {
    type Item = &'a mut ListItem<T>;
    type IntoIter = std::collections::vec_deque::IterMut<'a, ListItem<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter_mut()
    }
}

// C-style API compatibility layer
// 
// These functions provide direct equivalents to the original C functions
// for easier porting of existing code.

/// C-style list head structure for compatibility
#[repr(C)]
pub struct CListHead {
    list: *mut AudList<*mut std::ffi::c_void>,
}

/// C-style list node structure for compatibility
#[repr(C)]
pub struct CListNode {
    item: *mut std::ffi::c_void,
    priority: Priority,
}

/// C-compatible list initialization (equivalent to C ListInit)
/// 
/// # Safety
/// 
/// This function is unsafe because it works with raw pointers.
/// The caller must ensure the head pointer is valid.
pub unsafe fn list_init(head: *mut CListHead) {
    if head.is_null() {
        return;
    }
    
    let list = Box::new(AudList::<*mut std::ffi::c_void>::new());
    (*head).list = Box::into_raw(list);
}

/// C-compatible node initialization (equivalent to C ListNodeInit)
/// 
/// # Safety
/// 
/// This function is unsafe because it works with raw pointers.
/// The caller must ensure the node pointer is valid.
pub unsafe fn list_node_init(node: *mut CListNode) {
    if node.is_null() {
        return;
    }
    
    (*node).item = ptr::null_mut();
    (*node).priority = NORMAL_PRIORITY;
}

/// C-compatible ascending sort insertion (equivalent to C ListAddNodeSortAscending)
/// 
/// # Safety
/// 
/// This function is unsafe because it works with raw pointers.
/// The caller must ensure all pointers are valid.
pub unsafe fn list_add_node_sort_ascending(
    head: *mut CListHead,
    node: *mut CListNode,
) -> i32 {
    if head.is_null() || node.is_null() || (*head).list.is_null() {
        return -1;
    }
    
    let list = &mut *(*head).list;
    let item = ListItem::new((*node).priority, (*node).item);
    list.add_node_ascending(item) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_list() {
        let list: AudList<String> = AudList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_add_node_ascending() {
        let mut list = AudList::new();
        
        list.add_node_ascending(ListItem::new(10, "high"));
        list.add_node_ascending(ListItem::new(5, "medium"));
        list.add_node_ascending(ListItem::new(1, "low"));
        
        assert_eq!(list.len(), 3);
        assert_eq!(list.get_item(0).unwrap().priority(), 1);
        assert_eq!(list.get_item(1).unwrap().priority(), 5);
        assert_eq!(list.get_item(2).unwrap().priority(), 10);
    }

    #[test]
    fn test_add_node_descending() {
        let mut list = AudList::new();
        
        list.add_node(ListItem::new(1, "low"));
        list.add_node(ListItem::new(10, "high"));
        list.add_node(ListItem::new(5, "medium"));
        
        assert_eq!(list.len(), 3);
        assert_eq!(list.get_item(0).unwrap().priority(), 10);
        assert_eq!(list.get_item(1).unwrap().priority(), 5);
        assert_eq!(list.get_item(2).unwrap().priority(), 1);
    }

    #[test]
    fn test_merge_lists() {
        let mut list1 = AudList::new();
        let mut list2 = AudList::new();
        
        list1.add_node_ascending(ListItem::new(1, "list1-low"));
        list1.add_node_ascending(ListItem::new(10, "list1-high"));
        
        list2.add_node_ascending(ListItem::new(5, "list2-medium"));
        list2.add_node_ascending(ListItem::new(15, "list2-highest"));
        
        list1.merge(&mut list2);
        
        assert_eq!(list1.len(), 4);
        assert_eq!(list2.len(), 0);
        assert_eq!(list1.get_item(0).unwrap().priority(), 1);
        assert_eq!(list1.get_item(1).unwrap().priority(), 5);
        assert_eq!(list1.get_item(2).unwrap().priority(), 10);
        assert_eq!(list1.get_item(3).unwrap().priority(), 15);
    }

    #[test]
    fn test_remove_operations() {
        let mut list = AudList::new();
        
        list.add_node_ascending(ListItem::new(1, "first"));
        list.add_node_ascending(ListItem::new(2, "second"));
        list.add_node_ascending(ListItem::new(3, "third"));
        
        let removed = list.remove_at(1).unwrap();
        assert_eq!(removed.priority(), 2);
        assert_eq!(list.len(), 2);
        
        let first = list.remove_first().unwrap();
        assert_eq!(first.priority(), 1);
        
        let last = list.remove_last().unwrap();
        assert_eq!(last.priority(), 3);
        
        assert!(list.is_empty());
    }

    #[test]
    fn test_find_operations() {
        let mut list = AudList::new();
        
        list.add_node_ascending(ListItem::new(1, "first"));
        list.add_node_ascending(ListItem::new(2, "second"));
        list.add_node_ascending(ListItem::new(3, "third"));
        
        assert_eq!(list.find(&"second"), Some(1));
        assert_eq!(list.find(&"nonexistent"), None);
        
        let removed = list.remove_by_data(&"second").unwrap();
        assert_eq!(removed.data(), &"second");
        assert_eq!(list.len(), 2);
        
        assert!(list.remove_by_data(&"nonexistent").is_err());
    }

    #[test]
    fn test_iterator() {
        let mut list = AudList::new();
        
        list.add_node_ascending(ListItem::new(1, "first"));
        list.add_node_ascending(ListItem::new(2, "second"));
        list.add_node_ascending(ListItem::new(3, "third"));
        
        let priorities: Vec<Priority> = list.iter().map(|item| item.priority()).collect();
        assert_eq!(priorities, vec![1, 2, 3]);
        
        let data: Vec<&str> = list.into_iter().map(|item| *item.data()).collect();
        assert_eq!(data, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_error_handling() {
        let list: AudList<String> = AudList::new();
        
        assert!(matches!(list.get_item(0), Err(ListError::IndexOutOfBounds(0))));
        
        let mut empty_list: AudList<String> = AudList::new();
        assert!(matches!(empty_list.remove_first(), Err(ListError::EmptyList)));
        assert!(matches!(empty_list.remove_last(), Err(ListError::EmptyList)));
    }

    #[test]
    fn test_priority_constants() {
        assert_eq!(LOWEST_PRIORITY, i32::MIN);
        assert_eq!(HIGHEST_PRIORITY, i32::MAX);
        assert_eq!(NORMAL_PRIORITY, 0);
    }
}