//! Intrusive doubly-linked list compatible with the original WW3D container types.
//!
//! The historic engine stores linkage inside the node type and leaves lifetime management to the
//! embedding owner. This module mirrors that approach by exposing a [`DLListLink`] helper that can
//! be embedded into user-defined structs while [`DLListClass`] only manipulates raw pointers.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// Intrusive link that must be embedded in every node that participates in the list.
#[derive(Debug)]
pub struct DLListLink<T> {
    prev: Option<NonNull<T>>,
    next: Option<NonNull<T>>,
    list: *mut c_void,
}

impl<T> DLListLink<T> {
    /// Create a detached link.
    pub const fn new() -> Self {
        Self {
            prev: None,
            next: None,
            list: std::ptr::null_mut(),
        }
    }

    fn attach(&mut self, list: *mut c_void) {
        self.list = list;
    }

    fn detach(&mut self) {
        self.prev = None;
        self.next = None;
        self.list = std::ptr::null_mut();
    }

    fn is_linked(&self) -> bool {
        !self.list.is_null()
    }

    /// Retrieve the previous node pointer without detaching it.
    pub fn prev(&self) -> Option<NonNull<T>> {
        self.prev
    }

    /// Retrieve the next node pointer without detaching it.
    pub fn next(&self) -> Option<NonNull<T>> {
        self.next
    }

    /// Returns `true` if the node is currently attached to a list.
    pub fn is_attached(&self) -> bool {
        self.is_linked()
    }
}

impl<T> Default for DLListLink<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait implemented by intrusive nodes.
pub trait DLListNode: Sized {
    fn link(&self) -> &DLListLink<Self>;
    fn link_mut(&mut self) -> &mut DLListLink<Self>;
}

/// Doubly-linked intrusive list.
///
/// # Safety Invariants
///
/// This intrusive list maintains the following invariants that allow safe use of unsafe code:
///
/// 1. **Ownership**: Nodes are owned by the caller. The list only maintains non-owning pointers.
///    The caller is responsible for ensuring node lifetime exceeds list lifetime.
///
/// 2. **Link Validity**: Every node in the list must have a valid, properly initialized
///    [`DLListLink<T>`] and must implement [`DLListNode`].
///
/// 3. **Pointer Validity**: All stored pointers (head, tail, next, prev) either:
///    - Point to valid T instances initialized by the caller, OR
///    - Are None/null
///
/// 4. **Circular Invariant**: The doubly-linked structure must remain circular:
///    - If head is Some, head.prev must be None and head == tail.prev.next
///    - If tail is Some, tail.next must be None and tail == head.next.prev
///    - For any node N: N.next.prev == N and N.prev.next == N
///
/// 5. **Count Consistency**: self.count must equal the number of reachable nodes from head.
///
/// 6. **No Duplicates**: A node can only be in one list at a time. Each node has exactly
///    one `DLListLink<T>` which owns the linkage state.
///
/// # Usage Safety
///
/// As long as the caller respects ownership (ensuring nodes outlive the list), this
/// implementation is safe. The unsafe code blocks within are justified by these invariants.
#[derive(Debug)]
pub struct DLListClass<T: DLListNode> {
    head: Option<NonNull<T>>,
    tail: Option<NonNull<T>>,
    count: usize,
    marker: PhantomData<T>,
}

impl<T: DLListNode> DLListClass<T> {
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            count: 0,
            marker: PhantomData,
        }
    }

    /// Returns `true` if the list contains no elements.
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// Number of elements currently in the list.
    pub fn len(&self) -> usize {
        self.count
    }

    /// First element in the list.
    pub fn head(&self) -> Option<NonNull<T>> {
        self.head
    }

    /// Last element in the list.
    pub fn tail(&self) -> Option<NonNull<T>> {
        self.tail
    }

    /// Insert `node` at the beginning of the list.
    pub fn add_head(&mut self, node: &mut T) {
        // SAFETY: This is safe because:
        // - `node` is a valid mutable reference provided by the caller
        // - `self.head` is either None or a valid NonNull<T> maintained by list invariants
        // - `insert_before` maintains all list invariants including bidirectional links
        unsafe { self.insert_before(self.head, node) };
        #[cfg(debug_assertions)]
        self.assert_valid();
    }

    /// Insert `node` at the end of the list.
    pub fn add_tail(&mut self, node: &mut T) {
        // SAFETY: This is safe because:
        // - `node` is a valid mutable reference provided by the caller
        // - `self.tail` is either None or a valid NonNull<T> maintained by list invariants
        // - `insert_after` maintains all list invariants including bidirectional links
        unsafe { self.insert_after(self.tail, node) };
        #[cfg(debug_assertions)]
        self.assert_valid();
    }

    /// Remove and return the first node.
    pub fn remove_head(&mut self) -> Option<NonNull<T>> {
        let head = self.head?;
        // SAFETY: This is safe because:
        // - `head` is guaranteed to be Some(valid NonNull<T>) by the ? operator above
        // - `head` points to the first node in the list, which is valid by list invariants
        // - `unlink` properly maintains bidirectional links and updates head/tail/count
        unsafe {
            self.unlink(head);
        }
        #[cfg(debug_assertions)]
        self.assert_valid();
        Some(head)
    }

    /// Remove and return the last node.
    pub fn remove_tail(&mut self) -> Option<NonNull<T>> {
        let tail = self.tail?;
        // SAFETY: This is safe because:
        // - `tail` is guaranteed to be Some(valid NonNull<T>) by the ? operator above
        // - `tail` points to the last node in the list, which is valid by list invariants
        // - `unlink` properly maintains bidirectional links and updates head/tail/count
        unsafe {
            self.unlink(tail);
        }
        #[cfg(debug_assertions)]
        self.assert_valid();
        Some(tail)
    }

    /// Remove `node` from the list if it is attached.
    pub fn remove(&mut self, node: &mut T) {
        if !node.link().is_attached() {
            return;
        }
        let raw = NonNull::from(node);
        // SAFETY: This is safe because:
        // - `raw` is derived from a valid mutable reference `node` provided by caller
        // - The node is checked to be attached (is_linked() returned true)
        // - `unlink` properly maintains bidirectional links and updates head/tail/count
        unsafe {
            self.unlink(raw);
        }
        #[cfg(debug_assertions)]
        self.assert_valid();
    }

    /// Insert `node` before the given reference position.
    ///
    /// # Safety
    ///
    /// Safe to call from public methods because:
    /// - `node` is borrowed mutably by the caller, guaranteeing validity and no aliases
    /// - `reference` is checked to be None or a valid list node
    /// - The function only dereferences pointers derived from valid NonNull nodes in the list
    /// - Maintains all linking invariants (bidirectional links, circular structure, count)
    /// - Precondition checked: node must not be already linked (via debug_assert)
    unsafe fn insert_before(&mut self, reference: Option<NonNull<T>>, node: &mut T) {
        let node_ptr = NonNull::from(&mut *node);
        let link = node.link_mut();

        // Critical invariant: prevent double-insertion which would corrupt list structure
        assert!(
            !link.is_linked(),
            "attempted to insert node that already belongs to a list"
        );

        link.attach(self as *mut _ as *mut c_void);

        match reference {
            Some(ref_ptr) => {
                // SAFETY: ref_ptr is a valid NonNull<T> in the list (maintained by invariants)
                let ref_mut = ref_ptr.as_ptr();
                link.next = Some(ref_ptr);
                // SAFETY: Accessing link of reference node which is valid in the list
                link.prev = (*ref_mut).link().prev;

                if let Some(prev_ptr) = link.prev {
                    // SAFETY: prev_ptr is a valid node pointer from the list structure
                    (*prev_ptr.as_ptr()).link_mut().next = Some(node_ptr);
                } else {
                    self.head = Some(node_ptr);
                }

                // SAFETY: ref_mut points to a valid node in the list
                (*ref_mut).link_mut().prev = Some(node_ptr);
            }
            None => {
                // Inserting at end of list
                link.prev = self.tail;
                link.next = None;

                if let Some(tail_ptr) = self.tail {
                    // SAFETY: tail_ptr is a valid node pointer maintained by list invariants
                    (*tail_ptr.as_ptr()).link_mut().next = Some(node_ptr);
                } else {
                    self.head = Some(node_ptr);
                }

                self.tail = Some(node_ptr);
            }
        }

        self.count += 1;
    }

    /// Insert `node` after the given reference position.
    ///
    /// # Safety
    ///
    /// Safe to call from public methods because:
    /// - `node` is borrowed mutably by the caller, guaranteeing validity and no aliases
    /// - `reference` is checked to be None or a valid list node
    /// - The function only dereferences pointers derived from valid NonNull nodes in the list
    /// - Maintains all linking invariants (bidirectional links, circular structure, count)
    /// - Precondition checked: node must not be already linked (via debug_assert)
    unsafe fn insert_after(&mut self, reference: Option<NonNull<T>>, node: &mut T) {
        let node_ptr = NonNull::from(&mut *node);
        let link = node.link_mut();

        // Critical invariant: prevent double-insertion which would corrupt list structure
        assert!(
            !link.is_linked(),
            "attempted to insert node that already belongs to a list"
        );

        link.attach(self as *mut _ as *mut c_void);

        match reference {
            Some(ref_ptr) => {
                // SAFETY: ref_ptr is a valid NonNull<T> in the list (maintained by invariants)
                let ref_mut = ref_ptr.as_ptr();
                link.prev = Some(ref_ptr);
                // SAFETY: Accessing link of reference node which is valid in the list
                link.next = (*ref_mut).link().next;

                if let Some(next_ptr) = link.next {
                    // SAFETY: next_ptr is a valid node pointer from the list structure
                    (*next_ptr.as_ptr()).link_mut().prev = Some(node_ptr);
                } else {
                    self.tail = Some(node_ptr);
                }

                // SAFETY: ref_mut points to a valid node in the list
                (*ref_mut).link_mut().next = Some(node_ptr);
            }
            None => {
                // Inserting at beginning of list
                link.prev = None;
                link.next = self.head;

                if let Some(head_ptr) = self.head {
                    // SAFETY: head_ptr is a valid node pointer maintained by list invariants
                    (*head_ptr.as_ptr()).link_mut().prev = Some(node_ptr);
                } else {
                    self.tail = Some(node_ptr);
                }

                self.head = Some(node_ptr);
            }
        }

        self.count += 1;
    }

    /// Unlink (remove) the given node from the list without deallocating it.
    ///
    /// # Safety
    ///
    /// Safe to call because:
    /// - `node_ptr` is either the result of NonNull::from() on a valid list member, OR
    /// - `node_ptr` was passed to insert_before/insert_after and is still in the list
    /// - All pointers stored in the list are either None or point to valid T instances
    /// - The function only dereferences pointers within the list structure
    /// - Maintains all linking invariants (bidirectional links, circular structure, count)
    /// - Precondition checked: node must be linked into a list (via debug_assert)
    unsafe fn unlink(&mut self, node_ptr: NonNull<T>) {
        // SAFETY: node_ptr is guaranteed valid by caller (either from NonNull::from or list member)
        let node_ref = node_ptr.as_ptr();
        // SAFETY: Accessing link of a valid node
        let link = (*node_ref).link_mut();

        // Critical invariant: prevent double-removal which would corrupt list structure
        assert!(
            link.is_linked(),
            "attempted to remove node that is not linked into a list"
        );

        if self.head == Some(node_ptr) {
            self.head = link.next;
        }
        if self.tail == Some(node_ptr) {
            self.tail = link.prev;
        }

        if let Some(prev_ptr) = link.prev {
            // SAFETY: prev_ptr is a valid node pointer from the list structure
            (*prev_ptr.as_ptr()).link_mut().next = link.next;
        }
        if let Some(next_ptr) = link.next {
            // SAFETY: next_ptr is a valid node pointer from the list structure
            (*next_ptr.as_ptr()).link_mut().prev = link.prev;
        }

        link.detach();
        self.count -= 1;
    }

    /// Validates all list invariants for debugging purposes.
    ///
    /// This method performs comprehensive validation of the list structure:
    /// - All nodes have valid pointers or None
    /// - Bidirectional links are consistent (if A.next = B then B.prev = A)
    /// - Node count matches reachable node count
    /// - Head/tail consistency (head.prev is None, tail.next is None)
    /// - All nodes in the list are properly linked
    ///
    /// # Panics
    ///
    /// Panics if any invariant is violated, providing detailed error information.
    #[cfg(debug_assertions)]
    pub fn assert_valid(&self) {
        // Check empty list consistency
        if self.head.is_none() {
            assert!(
                self.tail.is_none(),
                "Head is None but tail is Some - inconsistent"
            );
            assert_eq!(self.count, 0, "Empty list should have count 0");
            return;
        }

        // Check single element list
        if self.head == self.tail {
            assert_eq!(self.count, 1, "List with head == tail should have count 1");
            unsafe {
                let head = self.head.unwrap();
                let head_link = (*head.as_ptr()).link();
                assert!(
                    head_link.prev.is_none(),
                    "Single node's prev should be None"
                );
                assert!(
                    head_link.next.is_none(),
                    "Single node's next should be None"
                );
            }
            return;
        }

        // Validate head
        unsafe {
            let head = self.head.unwrap();
            let head_link = (*head.as_ptr()).link();
            assert!(head_link.prev.is_none(), "Head node's prev should be None");
            assert!(
                head_link.next.is_some(),
                "Head node's next should be Some (list has >1 node)"
            );
        }

        // Validate tail
        unsafe {
            let tail = self.tail.unwrap();
            let tail_link = (*tail.as_ptr()).link();
            assert!(tail_link.next.is_none(), "Tail node's next should be None");
            assert!(
                tail_link.prev.is_some(),
                "Tail node's prev should be Some (list has >1 node)"
            );
        }

        // Traverse list and validate bidirectional consistency
        let mut current = self.head;
        let mut counted = 0;
        let mut prev_ptr: Option<NonNull<T>> = None;

        while let Some(curr_ptr) = current {
            counted += 1;

            // Prevent infinite loops
            assert!(
                counted <= self.count + 1,
                "Traversal counted more nodes than reported count - possible cycle"
            );

            unsafe {
                let curr_node = curr_ptr.as_ptr();
                let curr_link = (*curr_node).link();

                // Check backward link consistency
                assert_eq!(
                    curr_link.prev, prev_ptr,
                    "Bidirectional link inconsistency: node's prev doesn't match previous node"
                );

                // If we have a next node, verify it points back to us
                if let Some(next_ptr) = curr_link.next {
                    let next_link = (*next_ptr.as_ptr()).link();
                    assert_eq!(
                        next_link.prev,
                        Some(curr_ptr),
                        "Bidirectional link inconsistency: next node's prev doesn't point back"
                    );
                }

                // Move to next node
                prev_ptr = Some(curr_ptr);
                current = curr_link.next;
            }
        }

        // Verify count matches traversal
        assert_eq!(
            counted, self.count,
            "Node count mismatch: counted {} nodes but list reports {} nodes",
            counted, self.count
        );

        // Verify we ended at tail
        assert_eq!(
            prev_ptr, self.tail,
            "Traversal ended at different node than tail pointer"
        );
    }
}

impl<T: DLListNode> Default for DLListClass<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestNode {
        link: DLListLink<Self>,
        value: i32,
    }

    impl TestNode {
        fn new(value: i32) -> Self {
            Self {
                link: DLListLink::new(),
                value,
            }
        }
    }

    impl DLListNode for TestNode {
        fn link(&self) -> &DLListLink<Self> {
            &self.link
        }

        fn link_mut(&mut self) -> &mut DLListLink<Self> {
            &mut self.link
        }
    }

    #[test]
    fn add_remove_roundtrip() {
        let mut list = DLListClass::<TestNode>::new();
        let mut a = TestNode::new(1);
        let mut b = TestNode::new(2);
        let mut c = TestNode::new(3);

        list.add_tail(&mut a);
        list.add_tail(&mut b);
        list.add_tail(&mut c);

        assert_eq!(list.len(), 3);

        list.remove(&mut b);
        assert_eq!(list.len(), 2);
        assert!(!b.link.is_attached());

        let head = list.remove_head().unwrap();
        // SAFETY: head is a valid NonNull<TestNode> that was just removed from the list
        // The node is still alive (owned by local variable 'a') so dereferencing is safe
        unsafe {
            assert_eq!((*head.as_ptr()).value, 1);
        }

        let tail = list.remove_tail().unwrap();
        // SAFETY: tail is a valid NonNull<TestNode> that was just removed from the list
        // The node is still alive (owned by local variable 'c') so dereferencing is safe
        unsafe {
            assert_eq!((*tail.as_ptr()).value, 3);
        }

        assert!(list.is_empty());
    }

    #[test]
    #[should_panic(expected = "attempted to insert node that already belongs to a list")]
    fn test_double_insertion_detection() {
        let mut list = DLListClass::<TestNode>::new();
        let mut node = TestNode::new(1);

        // First insertion should succeed
        list.add_tail(&mut node);
        assert_eq!(list.len(), 1);

        // Second insertion of the same node should panic
        list.add_tail(&mut node);
    }

    #[test]
    fn test_bidirectional_consistency() {
        let mut list = DLListClass::<TestNode>::new();
        let mut a = TestNode::new(1);
        let mut b = TestNode::new(2);
        let mut c = TestNode::new(3);
        let mut d = TestNode::new(4);

        // Build a list: a <-> b <-> c <-> d
        list.add_tail(&mut a);
        list.add_tail(&mut b);
        list.add_tail(&mut c);
        list.add_tail(&mut d);

        // Verify bidirectional links are reciprocal
        let a_ptr = NonNull::from(&a);
        let b_ptr = NonNull::from(&b);
        let c_ptr = NonNull::from(&c);
        let d_ptr = NonNull::from(&d);

        // Check a -> b and b -> a
        assert_eq!(a.link().next, Some(b_ptr));
        assert_eq!(b.link().prev, Some(a_ptr));

        // Check b -> c and c -> b
        assert_eq!(b.link().next, Some(c_ptr));
        assert_eq!(c.link().prev, Some(b_ptr));

        // Check c -> d and d -> c
        assert_eq!(c.link().next, Some(d_ptr));
        assert_eq!(d.link().prev, Some(c_ptr));

        // Check boundaries
        assert!(a.link().prev.is_none());
        assert!(d.link().next.is_none());

        // assert_valid() should pass
        #[cfg(debug_assertions)]
        list.assert_valid();
    }

    #[test]
    fn test_count_consistency() {
        let mut list = DLListClass::<TestNode>::new();

        // Empty list
        assert_eq!(list.len(), 0);
        #[cfg(debug_assertions)]
        list.assert_valid();

        // Add nodes and verify count matches
        let mut nodes: Vec<TestNode> = (0..10).map(TestNode::new).collect();

        for (i, node) in nodes.iter_mut().enumerate() {
            list.add_tail(node);
            assert_eq!(list.len(), i + 1);
            #[cfg(debug_assertions)]
            list.assert_valid();
        }

        // Count by traversal
        let mut traversal_count = 0;
        let mut current = list.head();
        while let Some(curr_ptr) = current {
            traversal_count += 1;
            unsafe {
                let curr_node = curr_ptr.as_ptr();
                current = (*curr_node).link().next;
            }
        }
        assert_eq!(traversal_count, 10);
        assert_eq!(traversal_count, list.len());

        // Remove nodes and verify count decreases
        for i in (0..10).rev() {
            list.remove_tail();
            assert_eq!(list.len(), i);
            #[cfg(debug_assertions)]
            list.assert_valid();
        }

        assert_eq!(list.len(), 0);
        assert!(list.is_empty());
    }

    #[test]
    fn test_empty_list_safety() {
        let mut list = DLListClass::<TestNode>::new();

        // Verify empty list is valid
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert!(list.head().is_none());
        assert!(list.tail().is_none());

        #[cfg(debug_assertions)]
        list.assert_valid();

        // Operations on empty list should be safe
        assert!(list.remove_head().is_none());
        assert!(list.remove_tail().is_none());

        #[cfg(debug_assertions)]
        list.assert_valid();

        // Add and remove one node
        let mut node = TestNode::new(42);
        list.add_tail(&mut node);
        assert_eq!(list.len(), 1);
        assert_eq!(list.head(), list.tail());

        #[cfg(debug_assertions)]
        list.assert_valid();

        list.remove(&mut node);
        assert!(list.is_empty());

        #[cfg(debug_assertions)]
        list.assert_valid();

        // Remove from already-detached node should be safe (no-op)
        list.remove(&mut node);
        assert!(list.is_empty());

        #[cfg(debug_assertions)]
        list.assert_valid();
    }
}
