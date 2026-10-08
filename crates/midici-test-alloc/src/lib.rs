//! Counting global allocator for no-allocation test harnesses.
//!
//! Test-only support crate (not published). Product crates keep
//! `unsafe_code = "forbid"`; this crate is the single audited exception
//! because a [`GlobalAlloc`] implementation is inherently `unsafe`. Every
//! allocation call is forwarded to the standard [`System`] allocator
//! unchanged — the only added behavior is a live-allocation counter.
//!
//! A test binary opts in by declaring, in its own crate root (e.g. an
//! integration test file):
//!
//! ```ignore
//! #[global_allocator]
//! static GLOBAL: midici_test_alloc::Counting = midici_test_alloc::Counting;
//! ```
//!
//! and then samples [`live`] before and after the code under test:
//!
//! - strictly equal ⇒ the measured code performed no heap allocations;
//! - equal after draining produced output ⇒ transient allocations only
//!   (net-zero steady state).
//!
//! [`GlobalAlloc::realloc`] is always net-zero (one block in, one block
//! out) and is forwarded straight through; `alloc_zeroed` counts one live
//! allocation, like any other successful allocation.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

/// Outstanding allocations since process start (alloc − dealloc).
static LIVE: AtomicIsize = AtomicIsize::new(0);

/// Allocator that counts live allocations and forwards to [`System`].
pub struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            LIVE.fetch_add(1, Ordering::SeqCst);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        LIVE.fetch_sub(1, Ordering::SeqCst);
    }

    // Net-zero by the trait contract (a failed realloc leaves the original
    // block allocated; a successful one is one block in and one out).
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(layout);
        if !p.is_null() {
            LIVE.fetch_add(1, Ordering::SeqCst);
        }
        p
    }
}

/// Live allocation count (successful allocs without a matching dealloc).
pub fn live() -> isize {
    LIVE.load(Ordering::SeqCst)
}
