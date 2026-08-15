//! Stack-then-heap temporary allocator (`StackAllocator` in C++).

/// Temporary allocator: first request uses an in-struct `Vec` cap, overflow is heap.
pub struct StackAllocator<T, const STACK_COUNT: usize, const CONSTRUCT: bool> {
    storage: Option<Vec<T>>,
}

impl<T, const STACK_COUNT: usize, const CONSTRUCT: bool> StackAllocator<T, STACK_COUNT, CONSTRUCT> {
    pub fn new() -> Self {
        Self { storage: None }
    }

    /// Allocate `count` defaulted objects. Returns a slice into owned storage.
    pub fn alloc(&mut self, count: usize) -> Option<&mut [T]>
    where
        T: Default,
    {
        if self.storage.is_some() {
            return None;
        }
        let mut buf = Vec::with_capacity(count.max(1));
        if CONSTRUCT {
            buf.resize_with(count, T::default);
        } else {
            buf.resize_with(count, T::default);
        }
        self.storage = Some(buf);
        self.storage.as_mut().map(Vec::as_mut_slice)
    }

    /// Uninitialized path: same as [`Self::alloc`] because `Vec<T>` must be init.
    pub fn alloc_uninit(&mut self, count: usize) -> Option<&mut [T]>
    where
        T: Default,
    {
        self.alloc(count)
    }

    pub fn dealloc(&mut self) {
        self.storage = None;
    }

    pub fn used_stack(&self) -> bool {
        self.storage
            .as_ref()
            .is_some_and(|s| s.len() <= STACK_COUNT)
    }
}

impl<T, const STACK_COUNT: usize, const CONSTRUCT: bool> Default
    for StackAllocator<T, STACK_COUNT, CONSTRUCT>
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_then_release() {
        let mut allocator: StackAllocator<i32, 100, true> = StackAllocator::new();
        let slice = allocator.alloc(50).expect("stack alloc");
        assert_eq!(slice.len(), 50);
        assert!(allocator.used_stack());
        allocator.dealloc();
        let heap = allocator.alloc(200).expect("heap alloc");
        assert_eq!(heap.len(), 200);
        assert!(!allocator.used_stack());
        allocator.dealloc();
    }
}
