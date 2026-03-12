//! Reference-counted pointer wrapper (ported from WWLib ref_ptr.h).

use std::cmp::Ordering;

pub trait RefCounted {
    fn add_ref(&self);
    fn release_ref(&self);
}

#[derive(Default)]
pub struct RefCountPtr<T: RefCounted> {
    referent: Option<*mut T>,
}

impl<T: RefCounted> RefCountPtr<T> {
    pub fn new() -> Self {
        RefCountPtr { referent: None }
    }

    pub fn from_get(ptr: *mut T) -> Self {
        let mut result: RefCountPtr<T> = RefCountPtr { referent: None };
        result.set_ptr(ptr, ReferenceHandling::Get);
        result
    }

    pub fn from_peek(ptr: *mut T) -> Self {
        let mut result: RefCountPtr<T> = RefCountPtr { referent: None };
        result.set_ptr(ptr, ReferenceHandling::Peek);
        result
    }

    pub fn create_new(ptr: *mut T) -> Self {
        Self::from_get(ptr)
    }

    pub fn create_get(ptr: *mut T) -> Self {
        Self::from_get(ptr)
    }

    pub fn create_peek(ptr: *mut T) -> Self {
        Self::from_peek(ptr)
    }

    pub fn clear(&mut self) {
        if let Some(ptr) = self.referent.take() {
            unsafe {
                (*ptr).release_ref();
            }
        }
    }

    pub fn peek(&self) -> *mut T {
        self.referent.unwrap_or(std::ptr::null_mut())
    }

    pub fn as_ref(&self) -> Option<&T> {
        unsafe { self.referent.map(|ptr| &*ptr) }
    }

    pub fn as_mut(&mut self) -> Option<&mut T> {
        unsafe { self.referent.map(|ptr| &mut *ptr) }
    }

    fn set_ptr(&mut self, ptr: *mut T, handling: ReferenceHandling) {
        if let Some(existing) = self.referent {
            if existing == ptr {
                return;
            }
        }

        if let Some(existing) = self.referent.take() {
            unsafe {
                (*existing).release_ref();
            }
        }

        self.referent = if ptr.is_null() { None } else { Some(ptr) };
        if let Some(new_ptr) = self.referent {
            if handling == ReferenceHandling::Peek {
                unsafe {
                    (*new_ptr).add_ref();
                }
            }
        }
    }
}

impl<T: RefCounted> std::ops::Deref for RefCountPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref().expect("RefCountPtr is null")
    }
}

impl<T: RefCounted> std::ops::DerefMut for RefCountPtr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut().expect("RefCountPtr is null")
    }
}

impl<T: RefCounted> Clone for RefCountPtr<T> {
    fn clone(&self) -> Self {
        if let Some(ptr) = self.referent {
            unsafe {
                (*ptr).add_ref();
            }
        }
        RefCountPtr {
            referent: self.referent,
        }
    }
}

impl<T: RefCounted> Drop for RefCountPtr<T> {
    fn drop(&mut self) {
        if let Some(ptr) = self.referent {
            unsafe {
                (*ptr).release_ref();
            }
        }
    }
}

impl<T: RefCounted> PartialEq for RefCountPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.peek() == other.peek()
    }
}

impl<T: RefCounted> PartialOrd for RefCountPtr<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (self.peek() as usize).partial_cmp(&(other.peek() as usize))
    }
}

impl<T: RefCounted> Eq for RefCountPtr<T> {}

impl<T: RefCounted> Ord for RefCountPtr<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.peek() as usize).cmp(&(other.peek() as usize))
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ReferenceHandling {
    Get,
    Peek,
}
