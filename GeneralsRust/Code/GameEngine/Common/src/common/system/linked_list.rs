//! Owning priority list corresponding to the ordering and navigation in List.cpp.
//!
//! Nodes are owned by the list; a removed node is returned to its caller. Items
//! are owned as well (use a reference as `T` for externally owned items). A
//! cursor exclusively borrows its list, so its position cannot outlive the list,
//! become stale through another mutation, or be used with a different list.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    Ascending,
    #[default]
    Descending,
}

#[derive(Debug)]
pub struct LListNode<T> {
    item: Option<T>,
    priority: i32,
}

impl<T> LListNode<T> {
    pub fn new() -> Self { Self { item: None, priority: 0 } }
    pub fn with_item(item: T) -> Self { Self { item: Some(item), priority: 0 } }
    pub fn with_priority_and_item(priority: i32, item: T) -> Self {
        Self { item: Some(item), priority }
    }
    pub fn priority(&self) -> i32 { self.priority }
    pub fn set_priority(&mut self, priority: i32) { self.priority = priority; }
    pub fn item(&self) -> Option<&T> { self.item.as_ref() }
    pub fn item_mut(&mut self) -> Option<&mut T> { self.item.as_mut() }
    pub fn set_item(&mut self, item: T) { self.item = Some(item); }
    pub fn take_item(&mut self) -> Option<T> { self.item.take() }
}
impl<T> Default for LListNode<T> {
    fn default() -> Self { Self::new() }
}

struct Entry<T> {
    node: Box<LListNode<T>>,
    prev: Option<usize>,
    next: Option<usize>,
}

/// List-owned nodes, linked by private arena indices. Vacant slots are reused.
pub struct LList<T> {
    entries: Vec<Option<Entry<T>>>,
    free: Vec<usize>,
    first: Option<usize>,
    last: Option<usize>,
    len: usize,
    sort_mode: SortMode,
    add_to_end_of_group: bool,
}

impl<T> LList<T> {
    pub fn new() -> Self {
        Self { entries: Vec::new(), free: Vec::new(), first: None, last: None,
            len: 0, sort_mode: SortMode::default(), add_to_end_of_group: false }
    }
    fn entry(&self, index: usize) -> &Entry<T> { self.entries[index].as_ref().unwrap() }
    fn entry_mut(&mut self, index: usize) -> &mut Entry<T> { self.entries[index].as_mut().unwrap() }

    fn insert_between(&mut self, prev: Option<usize>, next: Option<usize>, node: Box<LListNode<T>>) -> usize {
        let entry = Entry { node, prev, next };
        let index = if let Some(index) = self.free.pop() {
            self.entries[index] = Some(entry);
            index
        } else {
            self.entries.push(Some(entry));
            self.entries.len() - 1
        };
        if let Some(prev) = prev { self.entry_mut(prev).next = Some(index); } else { self.first = Some(index); }
        if let Some(next) = next { self.entry_mut(next).prev = Some(index); } else { self.last = Some(index); }
        self.len += 1;
        index
    }

    fn unlink(&mut self, index: usize) -> Box<LListNode<T>> {
        let entry = self.entries[index].take().unwrap();
        if let Some(prev) = entry.prev { self.entry_mut(prev).next = entry.next; } else { self.first = entry.next; }
        if let Some(next) = entry.next { self.entry_mut(next).prev = entry.prev; } else { self.last = entry.prev; }
        self.len -= 1;
        self.free.push(index);
        entry.node
    }

    pub fn add_to_head(&mut self, node: Box<LListNode<T>>) { self.insert_between(None, self.first, node); }
    pub fn add_to_tail(&mut self, node: Box<LListNode<T>>) { self.insert_between(self.last, None, node); }

    /// Uses List.cpp's comparisons, which are opposite to the enum's C++ comments.
    /// Equal priorities are inserted at the beginning or end of their group.
    pub fn add(&mut self, node: Box<LListNode<T>>) {
        let priority = node.priority;
        if self.add_to_end_of_group {
            let mut current = self.last;
            while let Some(index) = current {
                let entry = self.entry(index);
                let matches = match self.sort_mode {
                    SortMode::Ascending => entry.node.priority >= priority,
                    SortMode::Descending => entry.node.priority <= priority,
                };
                if matches {
                    let next = entry.next;
                    self.insert_between(Some(index), next, node);
                    return;
                }
                current = entry.prev;
            }
            self.add_to_head(node);
        } else {
            let mut current = self.first;
            while let Some(index) = current {
                let entry = self.entry(index);
                let matches = match self.sort_mode {
                    SortMode::Ascending => entry.node.priority <= priority,
                    SortMode::Descending => entry.node.priority >= priority,
                };
                if matches {
                    let prev = entry.prev;
                    self.insert_between(prev, Some(index), node);
                    return;
                }
                current = entry.next;
            }
            self.add_to_tail(node);
        }
    }
    pub fn add_item_to_head(&mut self, item: T) { self.add_to_head(Box::new(LListNode::with_item(item))); }
    pub fn add_item_to_tail(&mut self, item: T) { self.add_to_tail(Box::new(LListNode::with_item(item))); }
    pub fn add_item(&mut self, priority: i32, item: T) { self.add(Box::new(LListNode::with_priority_and_item(priority, item))); }
    pub fn node_count(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn first_node(&self) -> Option<&LListNode<T>> { self.first.map(|i| self.entry(i).node.as_ref()) }
    pub fn last_node(&self) -> Option<&LListNode<T>> { self.last.map(|i| self.entry(i).node.as_ref()) }
    pub fn set_sort_mode(&mut self, mode: SortMode) { self.sort_mode = mode; }
    pub fn add_to_end_of_group(&mut self, yes: bool) { self.add_to_end_of_group = yes; }
    /// Returns the whole node, including an itemless node, if present.
    pub fn remove_first_node(&mut self) -> Option<Box<LListNode<T>>> { self.first.map(|i| self.unlink(i)) }
    /// Removes one node; `None` also represents an itemless first node.
    pub fn remove_first(&mut self) -> Option<T> { self.remove_first_node().and_then(|mut node| node.take_item()) }
    pub fn clear(&mut self) { while self.remove_first_node().is_some() {} }
    /// Appends the source nodes in their existing order, without sorting.
    pub fn merge(&mut self, other: &mut Self) {
        while let Some(node) = other.remove_first_node() { self.add_to_tail(node); }
    }
    pub fn find_item(&self, item: &T) -> bool where T: PartialEq { self.find_node(item).is_some() }
    pub fn find_node(&self, item: &T) -> Option<&LListNode<T>> where T: PartialEq {
        self.nodes().find(|node| node.item() == Some(item))
    }
    pub fn get_node(&self, index: i32) -> Option<&LListNode<T>> {
        usize::try_from(index).ok().and_then(|index| self.nodes().nth(index))
    }
    pub fn nodes(&self) -> LListNodes<'_, T> { LListNodes { list: self, current: self.first } }
    pub fn iter(&self) -> LListIter<'_, T> { LListIter { nodes: self.nodes() } }
    /// A cursor starting at the first node, or at the end if empty.
    pub fn cursor_front_mut(&mut self) -> LListCursor<'_, T> {
        let current = self.first;
        LListCursor { list: self, current }
    }
    /// A cursor starting at the last node, or at the end if empty.
    pub fn cursor_back_mut(&mut self) -> LListCursor<'_, T> {
        let current = self.last;
        LListCursor { list: self, current }
    }
}
impl<T> Default for LList<T> { fn default() -> Self { Self::new() } }
impl<T> Drop for LList<T> { fn drop(&mut self) { self.clear(); } }
impl<T: std::fmt::Debug> std::fmt::Debug for LList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.nodes()).finish()
    }
}

/// An exclusive, list-scoped position. At an end, `current` returns None;
/// `move_next`/`move_prev` from there start at the first/last node.
pub struct LListCursor<'a, T> { list: &'a mut LList<T>, current: Option<usize> }
impl<T> LListCursor<'_, T> {
    pub fn current(&self) -> Option<&LListNode<T>> { self.current.map(|i| self.list.entry(i).node.as_ref()) }
    pub fn current_mut(&mut self) -> Option<&mut LListNode<T>> {
        self.current.map(|i| self.list.entry_mut(i).node.as_mut())
    }
    pub fn move_next(&mut self) { self.current = match self.current { Some(i) => self.list.entry(i).next, None => self.list.first }; }
    pub fn move_prev(&mut self) { self.current = match self.current { Some(i) => self.list.entry(i).prev, None => self.list.last }; }
    pub fn loop_next(&mut self) { self.current = self.current.and_then(|i| self.list.entry(i).next).or(self.list.first); }
    pub fn loop_prev(&mut self) { self.current = self.current.and_then(|i| self.list.entry(i).prev).or(self.list.last); }
    /// Inserts before the current node, or at the tail at the end position.
    pub fn insert_before(&mut self, node: Box<LListNode<T>>) {
        let prev = self.current.map(|i| self.list.entry(i).prev).unwrap_or(self.list.last);
        self.list.insert_between(prev, self.current, node);
    }
    /// Inserts after the current node, or at the head at the end position.
    pub fn insert_after(&mut self, node: Box<LListNode<T>>) {
        let next = self.current.map(|i| self.list.entry(i).next).unwrap_or(self.list.first);
        self.list.insert_between(self.current, next, node);
    }
    /// Removes the current node and advances to its successor, if any.
    pub fn remove_current(&mut self) -> Option<Box<LListNode<T>>> {
        let index = self.current?;
        self.current = self.list.entry(index).next;
        Some(self.list.unlink(index))
    }
}

pub struct LListNodes<'a, T> { list: &'a LList<T>, current: Option<usize> }
impl<'a, T> Iterator for LListNodes<'a, T> {
    type Item = &'a LListNode<T>;
    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.list.entry(self.current?);
        self.current = entry.next;
        Some(entry.node.as_ref())
    }
}
pub struct LListIter<'a, T> { nodes: LListNodes<'a, T> }
impl<'a, T> Iterator for LListIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> { self.nodes.find_map(|node| node.item()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn cpp_sorted_insertions_and_equal_groups() {
        // List.cpp:83-118; its group flag is uninitialized, so set it explicitly.
        for (mode, end, expected) in [
            (SortMode::Ascending, false, [30, 21, 20, 10]),
            (SortMode::Ascending, true, [30, 20, 21, 10]),
            (SortMode::Descending, false, [10, 21, 20, 30]),
            (SortMode::Descending, true, [10, 20, 21, 30]),
        ] {
            let mut list = LList::new();
            list.set_sort_mode(mode);
            list.add_to_end_of_group(end);
            for (priority, value) in [(3, 30), (1, 10), (2, 20), (2, 21)] { list.add_item(priority, value); }
            assert_eq!(list.iter().copied().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn cpp_merge_concatenates_and_source_can_be_reused() {
        let mut a = LList::new();
        let mut b = LList::new();
        a.add_item_to_head(2);
        a.add_item_to_head(1);
        b.add_to_tail(Box::new(LListNode::new()));
        b.add_item_to_tail(3);
        let original = b.last_node().unwrap() as *const _;
        a.merge(&mut b);
        assert!(b.is_empty());
        assert_eq!(a.node_count(), 4);
        assert_eq!(a.iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(a.last_node().unwrap() as *const _, original);
        b.add_item_to_tail(4);
        assert_eq!(b.remove_first(), Some(4));
        assert_eq!(a.get_node(-1).map(|n| n.item()), None);
        assert!(a.get_node(2).unwrap().item().is_none());
        assert_eq!(a.find_node(&3).unwrap().item(), Some(&3));
    }

    #[test]
    fn cursor_navigation_insertion_and_removal_including_itemless() {
        let mut list = LList::new();
        list.add_item_to_tail(1);
        list.add_to_tail(Box::new(LListNode::new()));
        list.add_item_to_tail(3);
        {
            let mut cursor = list.cursor_front_mut();
            assert_eq!(cursor.current().unwrap().item(), Some(&1));
            cursor.move_prev();
            assert!(cursor.current().is_none());
            cursor.move_next();
            cursor.move_next();
            assert!(cursor.current().unwrap().item().is_none());
            cursor.insert_before(Box::new(LListNode::with_item(2)));
            assert!(cursor.remove_current().unwrap().item().is_none());
            assert_eq!(cursor.current().unwrap().item(), Some(&3));
            cursor.loop_next();
            assert_eq!(cursor.current().unwrap().item(), Some(&1));
            cursor.loop_prev();
            assert_eq!(cursor.current().unwrap().item(), Some(&3));
            cursor.insert_after(Box::new(LListNode::with_item(4)));
            assert_eq!(cursor.remove_current().unwrap().item(), Some(&3));
            assert_eq!(cursor.current().unwrap().item(), Some(&4));
        }
        assert_eq!(list.iter().copied().collect::<Vec<_>>(), [1, 2, 4]);
        assert_eq!(list.first_node().unwrap().item(), Some(&1));
        assert_eq!(list.last_node().unwrap().item(), Some(&4));
    }

    #[test]
    fn iteration_skips_all_itemless_positions_and_borrows_originals() {
        for slots in [vec![], vec![None, None], vec![None, Some(1), None, Some(2), None]] {
            let mut list = LList::new();
            for slot in &slots {
                list.add_to_tail(Box::new(match slot { Some(value) => LListNode::with_item(*value), None => LListNode::new() }));
            }
            assert_eq!(list.node_count(), slots.len());
            let mut iter = list.iter();
            let expected: Vec<_> = slots.iter().flatten().copied().collect();
            for value in &expected {
                let item = iter.next().unwrap();
                assert_eq!(item, value);
                assert!(std::ptr::eq(item, list.find_node(value).unwrap().item().unwrap()));
            }
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
            assert_eq!(list.nodes().count(), slots.len());
        }
    }

    #[test]
    fn clear_drop_and_removed_node_ownership() {
        struct Count<'a>(&'a Cell<usize>);
        impl Drop for Count<'_> { fn drop(&mut self) { self.0.set(self.0.get() + 1); } }
        let count = Cell::new(0);
        let mut list = LList::new();
        list.add_to_tail(Box::new(LListNode::new()));
        list.add_item_to_tail(Count(&count));
        list.add_item_to_tail(Count(&count));
        assert!(list.remove_first_node().unwrap().item().is_none());
        let saved = list.remove_first_node().unwrap();
        list.clear();
        assert_eq!(count.get(), 1);
        list.add_to_tail(saved);
        drop(list);
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn drop_visits_nodes_in_list_order_after_slot_reuse() {
        use std::cell::RefCell;
        struct Record<'a>(u8, &'a RefCell<Vec<u8>>);
        impl Drop for Record<'_> {
            fn drop(&mut self) { self.1.borrow_mut().push(self.0); }
        }
        let order = RefCell::new(Vec::new());
        let mut list = LList::new();
        list.add_item_to_tail(Record(1, &order));
        list.add_item_to_tail(Record(2, &order));
        drop(list.remove_first_node());
        list.add_item_to_tail(Record(3, &order));
        drop(list);
        assert_eq!(*order.borrow(), [1, 2, 3]);
    }
}
