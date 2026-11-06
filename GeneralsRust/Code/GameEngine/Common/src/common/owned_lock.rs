//! Owning lock guards for per-instance stores held behind `Arc`.
//!
//! The guards keep the `Arc` alive for as long as the guard exists, so the
//! free-function accessors (`get_xxx()`) can hand them out without borrowing
//! from a temporary `Arc` clone.

use parking_lot::{ArcMutexGuard, ArcRwLockReadGuard, ArcRwLockWriteGuard, RawMutex, RawRwLock};

pub type OwnedReadGuard<T> = ArcRwLockReadGuard<RawRwLock, T>;
pub type OwnedWriteGuard<T> = ArcRwLockWriteGuard<RawRwLock, T>;
pub type OwnedMutexGuard<T> = ArcMutexGuard<RawMutex, T>;
