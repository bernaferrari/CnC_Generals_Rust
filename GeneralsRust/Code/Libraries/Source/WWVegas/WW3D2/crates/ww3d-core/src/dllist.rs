//! Doubly-linked list compatible with WW3D `DLListClass` / `dllist.h`.
//!
//! C++ stored intrusive `NonNull` links inside caller-owned nodes. That is
//! unjustified `unsafe` in Rust: the list does not need pointer identity for
//! save/ABI. Nodes live in a [`slotmap::SlotMap`]; links are generational keys.

use slotmap::{DefaultKey, SlotMap};

/// Generational key for a list node (replaces C++ `T*`).
pub type NodeKey = DefaultKey;

/// Compatibility stub. C++ nodes embedded a `DLListLink`; the SlotMap owns nodes.
pub trait DLListNode {}

impl<T> DLListNode for T {}

/// Historic embedded link. Unused by the SlotMap list; kept so older embeds compile.
#[derive(Clone, Copy, Debug, Default)]
pub struct DLListLink {
    _private: (),
}

impl DLListLink {
    pub const fn new() -> Self {
        Self { _private: () }
    }

    pub fn is_attached(&self) -> bool {
        false
    }
}

#[derive(Debug)]
struct Node<T> {
    value: T,
    prev: Option<NodeKey>,
    next: Option<NodeKey>,
}

/// Doubly-linked list. Head/tail/next/prev are SlotMap keys, not raw pointers.
#[derive(Debug)]
pub struct DLListClass<T> {
    nodes: SlotMap<NodeKey, Node<T>>,
    head: Option<NodeKey>,
    tail: Option<NodeKey>,
}

impl<T> DLListClass<T> {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::new(),
            head: None,
            tail: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn head(&self) -> Option<NodeKey> {
        self.head
    }

    pub fn tail(&self) -> Option<NodeKey> {
        self.tail
    }

    pub fn get(&self, key: NodeKey) -> Option<&T> {
        self.nodes.get(key).map(|n| &n.value)
    }

    pub fn get_mut(&mut self, key: NodeKey) -> Option<&mut T> {
        self.nodes.get_mut(key).map(|n| &mut n.value)
    }

    pub fn next_of(&self, key: NodeKey) -> Option<NodeKey> {
        self.nodes.get(key).and_then(|n| n.next)
    }

    pub fn prev_of(&self, key: NodeKey) -> Option<NodeKey> {
        self.nodes.get(key).and_then(|n| n.prev)
    }

    pub fn add_head(&mut self, value: T) -> NodeKey {
        let old_head = self.head;
        let key = self.nodes.insert(Node {
            value,
            prev: None,
            next: old_head,
        });
        if let Some(head) = old_head {
            if let Some(node) = self.nodes.get_mut(head) {
                node.prev = Some(key);
            }
        } else {
            self.tail = Some(key);
        }
        self.head = Some(key);
        key
    }

    pub fn add_tail(&mut self, value: T) -> NodeKey {
        let old_tail = self.tail;
        let key = self.nodes.insert(Node {
            value,
            prev: old_tail,
            next: None,
        });
        if let Some(tail) = old_tail {
            if let Some(node) = self.nodes.get_mut(tail) {
                node.next = Some(key);
            }
        } else {
            self.head = Some(key);
        }
        self.tail = Some(key);
        key
    }

    pub fn remove(&mut self, key: NodeKey) -> Option<T> {
        let node = self.nodes.remove(key)?;
        match node.prev {
            Some(prev) => {
                if let Some(p) = self.nodes.get_mut(prev) {
                    p.next = node.next;
                }
            }
            None => self.head = node.next,
        }
        match node.next {
            Some(next) => {
                if let Some(n) = self.nodes.get_mut(next) {
                    n.prev = node.prev;
                }
            }
            None => self.tail = node.prev,
        }
        Some(node.value)
    }

    pub fn remove_head(&mut self) -> Option<T> {
        let head = self.head?;
        self.remove(head)
    }

    pub fn remove_tail(&mut self) -> Option<T> {
        let tail = self.tail?;
        self.remove(tail)
    }
}

impl<T> Default for DLListClass<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_remove_roundtrip() {
        let mut list = DLListClass::new();
        list.add_tail(1);
        let b = list.add_tail(2);
        list.add_tail(3);
        assert_eq!(list.len(), 3);

        assert_eq!(list.remove(b), Some(2));
        assert_eq!(list.len(), 2);
        assert_eq!(list.remove_head(), Some(1));
        assert_eq!(list.remove_tail(), Some(3));
        assert!(list.is_empty());
    }

    #[test]
    fn bidirectional_keys() {
        let mut list = DLListClass::new();
        let a = list.add_tail(1);
        let b = list.add_tail(2);
        let c = list.add_tail(3);
        assert_eq!(list.next_of(a), Some(b));
        assert_eq!(list.prev_of(b), Some(a));
        assert_eq!(list.next_of(b), Some(c));
        assert_eq!(list.prev_of(c), Some(b));
        assert!(list.prev_of(a).is_none());
        assert!(list.next_of(c).is_none());
    }

    #[test]
    fn empty_list_ops() {
        let mut list = DLListClass::<i32>::new();
        assert!(list.remove_head().is_none());
        assert!(list.remove_tail().is_none());
        let k = list.add_tail(42);
        assert_eq!(list.remove(k), Some(42));
        assert!(list.remove(k).is_none());
    }

    #[test]
    fn count_matches_inserts() {
        let mut list = DLListClass::new();
        let keys: Vec<_> = (0..10).map(|i| list.add_tail(i)).collect();
        assert_eq!(list.len(), 10);
        let mut n = 0;
        let mut cur = list.head();
        while let Some(k) = cur {
            n += 1;
            cur = list.next_of(k);
        }
        assert_eq!(n, 10);
        for k in keys.into_iter().rev() {
            list.remove(k);
        }
        assert!(list.is_empty());
    }
}
