//! Linked list implementation
//!
//! This module provides a doubly-linked list implementation that matches
//! the behavior of the C++ LList and LListNode classes from the original engine.

use std::fmt;
use std::ptr::NonNull;

/// Sorting mode for linked list insertion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    /// Lower priority numbers to front of list
    Ascending,
    /// Higher priority numbers to front of list
    Descending,
}

impl Default for SortMode {
    fn default() -> Self {
        SortMode::Descending
    }
}

/// A node in a linked list
///
/// Each node contains an optional item (generic data) and maintains
/// priority for sorted insertion.
pub struct LListNode<T> {
    /// Reference to the actual data item
    item: Option<T>,
    /// Priority for sorted insertion
    priority: i32,
    /// Next node in the list
    next: Option<NonNull<LListNode<T>>>,
    /// Previous node in the list
    prev: Option<NonNull<LListNode<T>>>,
    /// Removed nodes navigate to themselves without storing a movable self pointer.
    detached_self_loop: bool,
    /// Whether this node should auto-delete when removed
    auto_delete: bool,
    /// Whether this node is the list head/sentinel
    is_head: bool,
}

impl<T> LListNode<T> {
    /// Create a new list node
    pub fn new() -> Self {
        Self {
            item: None,
            priority: 0,
            next: None,
            prev: None,
            detached_self_loop: false,
            auto_delete: false,
            is_head: false,
        }
    }

    /// Create a new list node with an item
    pub fn with_item(item: T) -> Self {
        Self {
            item: Some(item),
            priority: 0,
            next: None,
            prev: None,
            detached_self_loop: false,
            auto_delete: false,
            is_head: false,
        }
    }

    /// Create a new list node with priority and item
    pub fn with_priority_and_item(priority: i32, item: T) -> Self {
        Self {
            item: Some(item),
            priority,
            next: None,
            prev: None,
            detached_self_loop: false,
            auto_delete: false,
            is_head: false,
        }
    }

    /// Set the priority of this node
    pub fn set_priority(&mut self, priority: i32) {
        self.priority = priority;
    }

    /// Get the priority of this node
    pub fn priority(&self) -> i32 {
        self.priority
    }

    /// Set the item for this node
    pub fn set_item(&mut self, item: T) {
        self.item = Some(item);
    }

    /// Get a reference to the item in this node
    pub fn item(&self) -> Option<&T> {
        self.item.as_ref()
    }

    /// Get a mutable reference to the item in this node
    pub fn item_mut(&mut self) -> Option<&mut T> {
        self.item.as_mut()
    }

    /// Take the item from this node, leaving None
    pub fn take_item(&mut self) -> Option<T> {
        self.item.take()
    }

    /// Set auto-delete flag
    pub fn auto_delete(&mut self) {
        self.auto_delete = true;
    }

    /// Check if this node is in a list (has connections)
    pub fn in_list(&self) -> bool {
        let self_ptr = NonNull::from(self);
        match self.prev {
            Some(prev) => prev != self_ptr,
            None => false,
        }
    }

    /// Check if this is a head/sentinel node
    pub fn is_head(&self) -> bool {
        self.is_head
    }

    /// Remove this node from the list it is in
    pub fn remove(&mut self) {
        if let (Some(prev), Some(next)) = (self.prev, self.next) {
            let self_ptr = NonNull::from(&mut *self);
            // A singleton's old self links may have been invalidated by this
            // exclusive receiver reborrow. Compare addresses, never dereference
            // those links; there are no other nodes to reconnect.
            if prev != self_ptr || next != self_ptr {
                // SAFETY: non-self neighbors are live and exclusively mutable
                // under the owning-list invariant or the raw insertion contract.
                unsafe {
                    (*prev.as_ptr()).next = Some(next);
                    (*next.as_ptr()).prev = Some(prev);
                }
            }
        }
        self.prev = None;
        self.next = None;
        self.detached_self_loop = true;
    }

    /// Insert a node before this node
    ///
    /// # Safety
    /// `self` must be freshly unlinked (a no-op), removed, or part of a live
    /// chain at stable addresses with valid reciprocal links. A removed anchor
    /// forms a two-node ring with `new_node`. `new_node` must be distinct from
    /// `self` and its neighbors, and must be an unlinked node at
    /// a stable address. The caller must have exclusive mutation access to all
    /// affected nodes: no outstanding references may alias the changed fields.
    /// When joining an owning `LList`, `new_node` must transfer unique ownership
    /// from a compatible `Box::into_raw` allocation; it must not be reclaimed
    /// elsewhere. Externally formed chains must keep every reachable node alive
    /// and unmoved for all references returned by safe navigation, and prevent
    /// mutation or reclamation while those shared references remain in use.
    /// All stored pointers must retain valid provenance for later navigation
    /// and mutation. Exclusive reborrows outside these link operations must not
    /// invalidate reachable back-pointers, even if allocations remain alive.
    /// These lifetime, provenance and aliasing guarantees continue after return.
    // SAFETY: the caller maintains stable, reciprocal links and exclusive
    // mutation access, including the transferred node's allocation ownership.
    pub unsafe fn insert_raw(&mut self, new_node: NonNull<LListNode<T>>) {
        let self_ptr = NonNull::from(&mut *self);
        let prev_ptr = match (*self_ptr.as_ptr()).prev {
            Some(prev) if prev == self_ptr => self_ptr,
            Some(prev) => prev,
            None if (*self_ptr.as_ptr()).detached_self_loop => self_ptr,
            None => return,
        };
        // Refresh the opposite neighbor's back-pointer after the exclusive
        // receiver reborrow; keep all writes in this raw pointer's lineage.
        let next_ptr = match (*self_ptr.as_ptr()).next {
            Some(next) if next != self_ptr => next,
            _ => self_ptr,
        };
        (*next_ptr.as_ptr()).prev = Some(self_ptr);
        (*self_ptr.as_ptr()).detached_self_loop = false;
        (*new_node.as_ptr()).detached_self_loop = false;
        (*new_node.as_ptr()).prev = Some(prev_ptr);
        (*new_node.as_ptr()).next = Some(self_ptr);
        (*prev_ptr.as_ptr()).next = Some(new_node);
        (*self_ptr.as_ptr()).prev = Some(new_node);
    }

    /// Append a node after this node
    ///
    /// # Safety
    /// `self` must be freshly unlinked (a no-op), removed, or part of a live
    /// chain at stable addresses with valid reciprocal links. A removed anchor
    /// forms a two-node ring with `new_node`. `new_node` must be distinct from
    /// `self` and its neighbors, and must be an unlinked node at
    /// a stable address. The caller must have exclusive mutation access to all
    /// affected nodes: no outstanding references may alias the changed fields.
    /// When joining an owning `LList`, `new_node` must transfer unique ownership
    /// from a compatible `Box::into_raw` allocation; it must not be reclaimed
    /// elsewhere. Externally formed chains must keep every reachable node alive
    /// and unmoved for all references returned by safe navigation, and prevent
    /// mutation or reclamation while those shared references remain in use.
    /// All stored pointers must retain valid provenance for later navigation
    /// and mutation. Exclusive reborrows outside these link operations must not
    /// invalidate reachable back-pointers, even if allocations remain alive.
    /// These lifetime, provenance and aliasing guarantees continue after return.
    // SAFETY: the caller maintains stable, reciprocal links and exclusive
    // mutation access, including the transferred node's allocation ownership.
    pub unsafe fn append_raw(&mut self, new_node: NonNull<LListNode<T>>) {
        let self_ptr = NonNull::from(&mut *self);
        let next_ptr = match (*self_ptr.as_ptr()).next {
            Some(next) if next == self_ptr => self_ptr,
            Some(next) => next,
            None if (*self_ptr.as_ptr()).detached_self_loop => self_ptr,
            None => return,
        };
        // Refresh the opposite neighbor's back-pointer after the exclusive
        // receiver reborrow; keep all writes in this raw pointer's lineage.
        let prev_ptr = match (*self_ptr.as_ptr()).prev {
            Some(prev) if prev != self_ptr => prev,
            _ => self_ptr,
        };
        (*prev_ptr.as_ptr()).next = Some(self_ptr);
        (*self_ptr.as_ptr()).detached_self_loop = false;
        (*new_node.as_ptr()).detached_self_loop = false;
        (*new_node.as_ptr()).prev = Some(self_ptr);
        (*new_node.as_ptr()).next = Some(next_ptr);
        (*next_ptr.as_ptr()).prev = Some(new_node);
        (*self_ptr.as_ptr()).next = Some(new_node);
    }

    /// Get the next node in the list, skipping the head node
    pub fn next_node(&self) -> Option<&LListNode<T>> {
        if self.detached_self_loop {
            return Some(self);
        }
        let next = self.next?;
        // SAFETY: next is a live neighbor; reading is_head and reborrowing
        // through &self keeps aliasing shared-only.
        unsafe {
            if (*next.as_ptr()).is_head() {
                None
            } else {
                Some(&*next.as_ptr())
            }
        }
    }

    /// Get the previous node in the list, skipping the head node
    pub fn prev_node(&self) -> Option<&LListNode<T>> {
        if self.detached_self_loop {
            return Some(self);
        }
        let prev = self.prev?;
        // SAFETY: prev is a live neighbor; reading is_head and reborrowing
        // through &self keeps aliasing shared-only.
        unsafe {
            if (*prev.as_ptr()).is_head() {
                None
            } else {
                Some(&*prev.as_ptr())
            }
        }
    }

    /// Get the next node, wrapping around the head when needed
    pub fn loop_next(&self) -> Option<&LListNode<T>> {
        if self.detached_self_loop {
            return Some(self);
        }
        let mut next = self.next?;
        // SAFETY: walk touches only live nodes reachable from this node via
        // links validated by list construction (circular with sentinel).
        unsafe {
            if (*next.as_ptr()).is_head() {
                let next_next = (*next.as_ptr()).next?;
                if (*next_next.as_ptr()).is_head() {
                    return None;
                }
                next = next_next;
            }
            Some(&*next.as_ptr())
        }
    }

    /// Get the previous node, wrapping around the head when needed
    pub fn loop_prev(&self) -> Option<&LListNode<T>> {
        if self.detached_self_loop {
            return Some(self);
        }
        let mut prev = self.prev?;
        // SAFETY: walk touches only live nodes reachable from this node via
        // links validated by list construction (circular with sentinel).
        unsafe {
            if (*prev.as_ptr()).is_head() {
                let prev_prev = (*prev.as_ptr()).prev?;
                if (*prev_prev.as_ptr()).is_head() {
                    return None;
                }
                prev = prev_prev;
            }
            Some(&*prev.as_ptr())
        }
    }
}

impl<T> Default for LListNode<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for LListNode<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LListNode")
            .field("item", &self.item)
            .field("priority", &self.priority)
            .field(
                "has_next",
                &(self.detached_self_loop || self.next.is_some()),
            )
            .field(
                "has_prev",
                &(self.detached_self_loop || self.prev.is_some()),
            )
            .field("auto_delete", &self.auto_delete)
            .finish()
    }
}

/// A doubly-linked list with priority-based sorting
///
/// This list maintains nodes in sorted order by priority and provides
/// operations for adding, removing, and iterating through items.
pub struct LList<T> {
    /// Head sentinel node
    head: NonNull<LListNode<T>>,
    /// Sorting mode for add operations
    sort_mode: SortMode,
    /// Whether to add nodes to the end of groups with the same priority
    add_to_end_of_group: bool,
}

impl<T> LList<T> {
    /// Create a new empty linked list
    pub fn new() -> Self {
        // Create head node on the heap
        let head_node = Box::new(LListNode::new());
        let head_ptr = NonNull::new(Box::into_raw(head_node)).unwrap();

        // Set up circular reference for empty list
        // SAFETY: head_ptr is the Box allocation above; these writes build
        // the empty-list sentinel self-loop before any other access.
        unsafe {
            (*head_ptr.as_ptr()).next = Some(head_ptr);
            (*head_ptr.as_ptr()).prev = Some(head_ptr);
            (*head_ptr.as_ptr()).is_head = true;
        }

        Self {
            head: head_ptr,
            sort_mode: SortMode::default(),
            add_to_end_of_group: false,
        }
    }

    /// Add a node to the head of the list
    pub fn add_to_head(&mut self, node: Box<LListNode<T>>) {
        let node_ptr = NonNull::new(Box::into_raw(node)).unwrap();

        // SAFETY: node_ptr was leaked from a Box one line up; head and first
        // are live members. The four writes splice it in after the head.
        unsafe {
            let head_ptr = self.head.as_ptr();
            let first_ptr = (*head_ptr).next.unwrap().as_ptr();

            // Insert after head (at beginning of actual list)
            (*node_ptr.as_ptr()).detached_self_loop = false;
            (*node_ptr.as_ptr()).next = (*head_ptr).next;
            (*node_ptr.as_ptr()).prev = Some(self.head);
            (*head_ptr).next = Some(node_ptr);
            (*first_ptr).prev = Some(node_ptr);
        }
    }

    /// Add a node to the tail of the list
    pub fn add_to_tail(&mut self, node: Box<LListNode<T>>) {
        let node_ptr = NonNull::new(Box::into_raw(node)).unwrap();

        // SAFETY: node_ptr was leaked from a Box one line up; head and last
        // are live members. The four writes splice it in before the head.
        unsafe {
            let head_ptr = self.head.as_ptr();
            let last_ptr = (*head_ptr).prev.unwrap().as_ptr();

            // Insert before head (at end of actual list)
            (*node_ptr.as_ptr()).detached_self_loop = false;
            (*node_ptr.as_ptr()).next = Some(self.head);
            (*node_ptr.as_ptr()).prev = (*head_ptr).prev;
            (*head_ptr).prev = Some(node_ptr);
            (*last_ptr).next = Some(node_ptr);
        }
    }

    /// Add a node in sorted order by priority
    pub fn add(&mut self, node: Box<LListNode<T>>) {
        let new_priority = node.priority();

        if self.add_to_end_of_group {
            // Find position by scanning backwards from head
            let mut current = self.head;
            // SAFETY: scan dereferences only nodes reachable from the head
            // sentinel, which stay alive while &mut self is held.
            unsafe {
                while let Some(prev_ptr) = (*current.as_ptr()).prev {
                    if prev_ptr == self.head {
                        break;
                    }

                    let prev_priority = (*prev_ptr.as_ptr()).priority;
                    let should_insert = match self.sort_mode {
                        SortMode::Ascending => prev_priority >= new_priority,
                        SortMode::Descending => prev_priority <= new_priority,
                    };

                    if should_insert {
                        self.insert_after(prev_ptr, node);
                        return;
                    }

                    current = prev_ptr;
                }
            }

            // Insert at head if no suitable position found
            self.add_to_head(node);
        } else {
            // Find position by scanning forwards from head
            let mut current = self.head;
            // SAFETY: scan dereferences only nodes reachable from the head
            // sentinel, which stay alive while &mut self is held.
            unsafe {
                while let Some(next_ptr) = (*current.as_ptr()).next {
                    if next_ptr == self.head {
                        break;
                    }

                    let next_priority = (*next_ptr.as_ptr()).priority;
                    let should_insert = match self.sort_mode {
                        SortMode::Ascending => next_priority <= new_priority,
                        SortMode::Descending => next_priority >= new_priority,
                    };

                    if should_insert {
                        self.insert_before(next_ptr, node);
                        return;
                    }

                    current = next_ptr;
                }
            }

            // Insert at tail if no suitable position found
            self.add_to_tail(node);
        }
    }

    /// Insert a node after the specified node
    fn insert_after(&mut self, after: NonNull<LListNode<T>>, node: Box<LListNode<T>>) {
        let node_ptr = NonNull::new(Box::into_raw(node)).unwrap();

        // SAFETY: after and its successor are live nodes owned by this list;
        // the writes splice the freshly-leaked node between them.
        unsafe {
            let after_ptr = after.as_ptr();
            let next_ptr = (*after_ptr).next.unwrap().as_ptr();

            (*node_ptr.as_ptr()).detached_self_loop = false;
            (*node_ptr.as_ptr()).next = (*after_ptr).next;
            (*node_ptr.as_ptr()).prev = Some(after);
            (*after_ptr).next = Some(node_ptr);
            (*next_ptr).prev = Some(node_ptr);
        }
    }

    /// Insert a node before the specified node
    fn insert_before(&mut self, before: NonNull<LListNode<T>>, node: Box<LListNode<T>>) {
        let node_ptr = NonNull::new(Box::into_raw(node)).unwrap();

        // SAFETY: before and its predecessor are live nodes owned by this
        // list; the writes splice the freshly-leaked node between them.
        unsafe {
            let before_ptr = before.as_ptr();
            let prev_ptr = (*before_ptr).prev.unwrap().as_ptr();

            (*node_ptr.as_ptr()).detached_self_loop = false;
            (*node_ptr.as_ptr()).next = Some(before);
            (*node_ptr.as_ptr()).prev = (*before_ptr).prev;
            (*before_ptr).prev = Some(node_ptr);
            (*prev_ptr).next = Some(node_ptr);
        }
    }

    /// Add an item to the head of the list
    pub fn add_item_to_head(&mut self, item: T) {
        let mut node = Box::new(LListNode::with_item(item));
        node.auto_delete();
        self.add_to_head(node);
    }

    /// Add an item to the tail of the list
    pub fn add_item_to_tail(&mut self, item: T) {
        let mut node = Box::new(LListNode::with_item(item));
        node.auto_delete();
        self.add_to_tail(node);
    }

    /// Add an item with priority in sorted order
    pub fn add_item(&mut self, priority: i32, item: T) {
        let mut node = Box::new(LListNode::with_priority_and_item(priority, item));
        node.auto_delete();
        self.add(node);
    }

    /// Get the number of nodes in the list
    pub fn node_count(&self) -> usize {
        let mut count = 0;
        let mut current = self.first_node();
        while let Some(node) = current {
            count += 1;
            current = node.next_node();
        }
        count
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        // SAFETY: reads only the head sentinel's prev field; the sentinel
        // outlives &self.
        unsafe {
            let head_ptr = self.head.as_ptr();
            let self_ptr = self.head;
            (*head_ptr).prev == Some(self_ptr)
        }
    }

    /// Get the first node in the list (not the head sentinel)
    pub fn first_node(&self) -> Option<&LListNode<T>> {
        // SAFETY: head sentinel is alive for &self; first is either the
        // sentinel itself (empty) or a live owned node.
        unsafe {
            let head_ptr = self.head.as_ptr();
            let first = (*head_ptr).next?;

            // Check if we're pointing back to head (empty list)
            if first == self.head {
                None
            } else {
                Some(&*first.as_ptr())
            }
        }
    }

    /// Get the last node in the list
    pub fn last_node(&self) -> Option<&LListNode<T>> {
        // SAFETY: head sentinel is alive for &self; last is either the
        // sentinel itself (empty) or a live owned node.
        unsafe {
            let head_ptr = self.head.as_ptr();
            let last = (*head_ptr).prev?;

            // Check if we're pointing back to head (empty list)
            if last == self.head {
                None
            } else {
                Some(&*last.as_ptr())
            }
        }
    }

    /// Clear all nodes from the list
    pub fn clear(&mut self) {
        while !self.is_empty() {
            // An itemless node still owns an allocation and may have successors.
            drop(self.remove_first());
        }
    }

    /// Remove and return the first item from the list
    pub fn remove_first(&mut self) -> Option<T> {
        // SAFETY: unlinks a live non-sentinel node whose neighbors stay put,
        // then reclaims exactly the Box leaked at add time via from_raw.
        unsafe {
            let head_ptr = self.head.as_ptr();
            let first = (*head_ptr).next?;

            if first == self.head {
                return None; // Empty list
            }

            // Unlink the node
            let first_ptr = first.as_ptr();
            let next = (*first_ptr).next.unwrap();
            let prev = (*first_ptr).prev.unwrap();

            (*prev.as_ptr()).next = Some(next);
            (*next.as_ptr()).prev = Some(prev);

            // Convert back to Box and extract item
            let node = Box::from_raw(first_ptr);
            node.item
        }
    }

    /// Set the sorting mode
    pub fn set_sort_mode(&mut self, mode: SortMode) {
        self.sort_mode = mode;
    }

    /// Set whether to add nodes to the end of groups with the same priority
    pub fn add_to_end_of_group(&mut self, yes: bool) {
        self.add_to_end_of_group = yes;
    }

    /// Merge another list into this one
    pub fn merge(&mut self, other: &mut LList<T>) {
        if other.is_empty() {
            return;
        }

        // SAFETY: both lists hold live sentinels under &mut self; splicing
        // moves other's whole chain into self, then re-self-loops other's
        // sentinel so it stays a valid empty list. No node is dropped here
        unsafe {
            let self_head = self.head.as_ptr();
            let other_head = other.head.as_ptr();

            // Link the end of self to the beginning of other
            let self_last = (*self_head).prev.unwrap();
            let other_first = (*other_head).next.unwrap();
            let other_last = (*other_head).prev.unwrap();

            (*self_last.as_ptr()).next = Some(other_first);
            (*other_first.as_ptr()).prev = Some(self_last);
            (*other_last.as_ptr()).next = Some(self.head);
            (*self_head).prev = Some(other_last);

            // Clear the other list's head
            (*other_head).next = Some(other.head);
            (*other_head).prev = Some(other.head);
        }
    }

    /// Find an item in the list
    pub fn find_item(&self, item: &T) -> bool
    where
        T: PartialEq,
    {
        for node_item in self.iter() {
            if node_item == item {
                return true;
            }
        }
        false
    }

    /// Find the first node containing the given item
    pub fn find_node(&self, item: &T) -> Option<&LListNode<T>>
    where
        T: PartialEq,
    {
        let mut current = self.first_node();
        while let Some(node) = current {
            if let Some(node_item) = node.item() {
                if node_item == item {
                    return Some(node);
                }
            }
            current = node.next_node();
        }
        None
    }

    /// Get a node by zero-based index
    pub fn get_node(&self, mut index: i32) -> Option<&LListNode<T>> {
        let mut current = self.first_node();
        while let Some(node) = current {
            if index == 0 {
                return Some(node);
            }
            index -= 1;
            current = node.next_node();
        }
        None
    }

    /// Create an iterator over the items in the list
    pub fn iter(&self) -> LListIter<'_, T> {
        LListIter::new(self)
    }
}

impl<T> Default for LList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for LList<T> {
    fn drop(&mut self) {
        // Clear all nodes first
        self.clear();

        // Then drop the head node
        // SAFETY: clear() emptied the chain, so this from_raw recovers
        // exactly the sentinel Box leaked in new().
        unsafe {
            let _ = Box::from_raw(self.head.as_ptr());
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for LList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LList")
            .field("count", &self.node_count())
            .field("sort_mode", &self.sort_mode)
            .field("add_to_end_of_group", &self.add_to_end_of_group)
            .finish()
    }
}

/// Iterator for linked list
pub struct LListIter<'a, T> {
    current: Option<NonNull<LListNode<T>>>,
    head: NonNull<LListNode<T>>,
    _phantom: std::marker::PhantomData<&'a T>,
}

impl<'a, T> LListIter<'a, T> {
    fn new(list: &'a LList<T>) -> Self {
        // SAFETY: reads the head sentinel's next link; sentinel outlives
        // the iterator borrow of the list.
        unsafe {
            let head_ptr = list.head.as_ptr();
            let first = (*head_ptr).next;

            Self {
                current: if first == Some(list.head) {
                    None
                } else {
                    first
                },
                head: list.head,
                _phantom: std::marker::PhantomData,
            }
        }
    }
}

impl<'a, T> Iterator for LListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: current nodes are list-owned and alive for 'a; advancing
        // follows the same links without dropping or mutating nodes.
        unsafe {
            while let Some(current) = self.current {
                let current_ptr = current.as_ptr();

                // Itemless nodes still have successors; advance before skipping them.
                let next = (*current_ptr).next;
                self.current = if next == Some(self.head) { None } else { next };

                if let Some(item) = (*current_ptr).item.as_ref() {
                    return Some(item);
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = LListNode::<i32>::new();
        assert_eq!(node.priority(), 0);
        assert!(node.item().is_none());
        assert!(!node.in_list());

        let node_with_item = LListNode::with_item(42);
        assert_eq!(node_with_item.item(), Some(&42));

        let node_with_priority = LListNode::with_priority_and_item(10, 42);
        assert_eq!(node_with_priority.priority(), 10);
        assert_eq!(node_with_priority.item(), Some(&42));
    }

    #[test]
    fn test_list_creation() {
        let list = LList::<i32>::new();
        assert_eq!(list.node_count(), 0);
        assert!(list.is_empty());
        assert!(list.first_node().is_none());
        assert!(list.last_node().is_none());
    }

    #[test]
    fn test_add_to_head_tail() {
        let mut list = LList::new();

        list.add_item_to_head(1);
        list.add_item_to_head(2);
        assert_eq!(list.node_count(), 2);

        list.add_item_to_tail(3);
        assert_eq!(list.node_count(), 3);

        // Should be: 2, 1, 3
        let items: Vec<_> = list.iter().cloned().collect();
        assert_eq!(items, vec![2, 1, 3]);
    }

    #[test]
    fn test_sorted_insertion() {
        // List.cpp:83-118 is the ordering oracle, including equal priorities.
        // Its constructor does not initialize the group flag; set it explicitly.
        for (mode, end, expected) in [
            (SortMode::Ascending, false, [30, 21, 20, 10]),
            (SortMode::Ascending, true, [30, 20, 21, 10]),
            (SortMode::Descending, false, [10, 21, 20, 30]),
            (SortMode::Descending, true, [10, 20, 21, 30]),
        ] {
            let mut list = LList::new();
            list.set_sort_mode(mode);
            list.add_to_end_of_group(end);
            for (priority, item) in [(3, 30), (1, 10), (2, 20), (2, 21)] {
                list.add_item(priority, item);
            }
            assert_eq!(list.iter().copied().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn test_remove_first() {
        let mut list = LList::new();

        list.add_item_to_tail(1);
        list.add_item_to_tail(2);
        list.add_item_to_tail(3);

        assert_eq!(list.remove_first(), Some(1));
        assert_eq!(list.remove_first(), Some(2));
        assert_eq!(list.node_count(), 1);

        assert_eq!(list.remove_first(), Some(3));
        assert!(list.is_empty());
        assert_eq!(list.remove_first(), None);
    }

    #[test]
    fn test_clear() {
        let mut list = LList::new();

        list.add_item_to_tail(1);
        list.add_item_to_tail(2);
        list.add_item_to_tail(3);

        assert_eq!(list.node_count(), 3);
        list.clear();
        assert!(list.is_empty());
    }

    #[test]
    fn test_find_item() {
        let mut list = LList::new();

        list.add_item_to_tail(1);
        list.add_item_to_tail(2);
        list.add_item_to_tail(3);

        assert!(list.find_item(&2));
        assert!(!list.find_item(&4));
    }

    #[test]
    fn test_merge() {
        let mut list1 = LList::new();
        let mut list2 = LList::new();

        list1.add_item_to_tail(1);
        list1.add_item_to_tail(2);

        list2.add_item_to_tail(3);
        list2.add_item_to_tail(4);

        list1.merge(&mut list2);

        assert_eq!(list1.node_count(), 4);
        assert_eq!(list2.node_count(), 0);

        let items: Vec<_> = list1.iter().cloned().collect();
        assert_eq!(items, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_iterator() {
        let mut list = LList::new();

        for i in 1..=5 {
            list.add_item_to_tail(i);
        }

        let items: Vec<_> = list.iter().cloned().collect();
        assert_eq!(items, vec![1, 2, 3, 4, 5]);

        // Test iterator is consumed properly
        let count = list.iter().count();
        assert_eq!(count, 5);
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn removed_node_survives_moves_and_reinsertion() {
        let mut boxed = Box::new(LListNode::with_item(7));
        assert!(boxed.next_node().is_none());
        boxed.remove();
        let mut moved = *boxed;
        assert!(!moved.in_list());
        assert!(std::ptr::eq(moved.next_node().unwrap(), &moved));
        assert!(std::ptr::eq(moved.prev_node().unwrap(), &moved));
        assert!(std::ptr::eq(moved.loop_next().unwrap(), &moved));
        assert!(std::ptr::eq(moved.loop_prev().unwrap(), &moved));
        moved.remove();
        let mut list = LList::new();
        list.add_to_tail(Box::new(moved));
        assert!(list.first_node().unwrap().next_node().is_none());
        assert_eq!(list.remove_first(), Some(7));
        assert!(list.is_empty());
    }

    #[test]
    fn clear_and_drop_consume_itemless_nodes() {
        struct Count(Rc<Cell<usize>>);
        impl Drop for Count {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        for slots in [
            [false, true, true],
            [true, false, true],
            [true, true, false],
            [false, false, false],
        ] {
            for explicit in [false, true] {
                let count = Rc::new(Cell::new(0));
                let mut list = LList::new();
                for has_item in slots {
                    list.add_to_tail(Box::new(if has_item {
                        LListNode::with_item(Count(count.clone()))
                    } else {
                        LListNode::new()
                    }));
                }
                let expected = slots.iter().filter(|&&v| v).count();
                if explicit {
                    list.clear();
                    assert!(list.is_empty());
                    assert_eq!(list.node_count(), 0);
                    assert_eq!(count.get(), expected);
                    list.add_item_to_tail(Count(count.clone()));
                }
                drop(list);
                assert_eq!(count.get(), expected + usize::from(explicit));
            }
        }
    }
}

#[cfg(test)]
mod iteration_tests {
    use super::*;

    fn list_with_slots(slots: &[Option<i32>]) -> LList<i32> {
        let mut list = LList::new();
        for slot in slots {
            list.add_to_tail(Box::new(match slot {
                Some(item) => LListNode::with_item(*item),
                None => LListNode::new(),
            }));
        }
        list
    }

    fn assert_iteration(slots: &[Option<i32>], expected: &[i32]) {
        let list = list_with_slots(slots);
        let mut iter = list.iter();
        let mut held = Vec::new();
        for value in expected {
            let item = iter.next().expect("itemless nodes must not end iteration");
            assert_eq!(item, value);
            // Yield the original borrowed item, not a replacement or copy.
            assert!(std::ptr::eq(
                item,
                list.find_node(value).unwrap().item().unwrap()
            ));
            held.push(item);
        }
        for _ in 0..3 {
            assert_eq!(iter.next(), None);
        }
        assert_eq!(held.iter().map(|&&item| item).collect::<Vec<_>>(), expected);
        assert_eq!(list.iter().copied().collect::<Vec<_>>(), expected);
        assert_eq!(list.node_count() as usize, slots.len());
    }

    #[test]
    fn itemless_head() {
        assert_iteration(&[None, Some(1), Some(2)], &[1, 2]);
    }

    #[test]
    fn itemless_middle() {
        assert_iteration(&[Some(1), None, Some(2)], &[1, 2]);
    }

    #[test]
    fn itemless_tail() {
        assert_iteration(&[Some(1), Some(2), None], &[1, 2]);
    }

    #[test]
    fn all_itemless() {
        assert_iteration(&[None, None, None], &[]);
    }

    #[test]
    fn empty_end_is_repeatable() {
        assert_iteration(&[], &[]);
    }

    #[test]
    fn consecutive_itemless_nodes() {
        assert_iteration(
            &[None, None, Some(1), None, None, Some(2), None, None],
            &[1, 2],
        );
    }

    #[test]
    fn find_item_after_itemless_head() {
        let list = list_with_slots(&[None, Some(1), None, Some(2), None]);
        assert!(list.find_item(&1));
        assert!(list.find_item(&2));
        assert!(!list.find_item(&3));
    }

    #[test]
    fn find_item_after_itemless_middle() {
        let list = list_with_slots(&[Some(1), None, None, Some(2)]);
        assert!(list.find_item(&1));
        assert!(list.find_item(&2));
        assert!(!list.find_item(&3));
    }

    #[test]
    fn find_item_missing_from_itemless_lists() {
        for slots in [&[None, None][..], &[Some(1), None][..], &[][..]] {
            assert!(!list_with_slots(slots).find_item(&3));
        }
    }

    #[test]
    fn merged_itemless_lists_preserve_order_and_source_reuse() {
        let mut list = list_with_slots(&[None, Some(1), None]);
        let mut other = list_with_slots(&[None, Some(2), None]);
        list.merge(&mut other);
        assert!(other.is_empty());
        assert_eq!(list.node_count(), 6);
        assert_eq!(list.iter().copied().collect::<Vec<_>>(), [1, 2]);
        assert!(list.find_item(&2));
        assert!(!list.find_item(&3));
        other.add_to_tail(Box::new(LListNode::new()));
        other.add_item_to_tail(3);
        assert_eq!(other.iter().copied().collect::<Vec<_>>(), [3]);
        assert_eq!(list.iter().copied().collect::<Vec<_>>(), [1, 2]);
    }
}
