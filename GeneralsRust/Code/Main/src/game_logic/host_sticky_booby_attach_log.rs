//! Frame-local sticky-bomb / booby-trap attach follow logs for GW shadow parity.

use super::ObjectId;
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct StickyFollowEvent {
    pub id: ObjectId,
    pub pos: Vec3,
}

#[derive(Debug, Clone)]
pub struct StickyDestroyEvent {
    pub id: ObjectId,
}

#[derive(Debug, Clone)]
pub struct BoobyFollowEvent {
    pub id: ObjectId,
    pub pos: Vec3,
}

#[derive(Debug, Clone)]
pub struct BoobyDestroyEvent {
    pub id: ObjectId,
}

thread_local! {
    static STICKY_FOLLOW: RefCell<Vec<StickyFollowEvent>> = RefCell::new(Vec::new());
    static STICKY_DESTROY: RefCell<Vec<StickyDestroyEvent>> = RefCell::new(Vec::new());
    static BOOBY_FOLLOW: RefCell<Vec<BoobyFollowEvent>> = RefCell::new(Vec::new());
    static BOOBY_DESTROY: RefCell<Vec<BoobyDestroyEvent>> = RefCell::new(Vec::new());
}

pub fn record_sticky_follow(ev: StickyFollowEvent) {
    STICKY_FOLLOW.with(|l| l.borrow_mut().push(ev));
}
pub fn record_sticky_destroy(ev: StickyDestroyEvent) {
    STICKY_DESTROY.with(|l| l.borrow_mut().push(ev));
}
pub fn record_booby_follow(ev: BoobyFollowEvent) {
    BOOBY_FOLLOW.with(|l| l.borrow_mut().push(ev));
}
pub fn record_booby_destroy(ev: BoobyDestroyEvent) {
    BOOBY_DESTROY.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_sticky_follows() -> Vec<StickyFollowEvent> {
    STICKY_FOLLOW.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn drain_sticky_destroys() -> Vec<StickyDestroyEvent> {
    STICKY_DESTROY.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn drain_booby_follows() -> Vec<BoobyFollowEvent> {
    BOOBY_FOLLOW.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn drain_booby_destroys() -> Vec<BoobyDestroyEvent> {
    BOOBY_DESTROY.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    STICKY_FOLLOW.with(|l| l.borrow_mut().clear());
    STICKY_DESTROY.with(|l| l.borrow_mut().clear());
    BOOBY_FOLLOW.with(|l| l.borrow_mut().clear());
    BOOBY_DESTROY.with(|l| l.borrow_mut().clear());
}
