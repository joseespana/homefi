//! "Nothing allocates in the hot loop": reading a long track allocates exactly as much as a
//! short one (the buffers are made once per track), counted with a global allocator.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use homefi_disc::{read_track, ImageSource, ReadPlan, SectorSource};

struct Counting;
thread_local! {
    /// Per thread, so the test harness's own threads can't add noise.
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    ALLOCATIONS.with(|n| n.set(n.get() + 1));
}

// SAFETY: forwards every call unchanged to the system allocator, only counting.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocations_to_read(sectors: u32) -> usize {
    let mut image = ImageSource::synthetic(&[sectors]);
    let toc = image.toc().unwrap();
    let before = ALLOCATIONS.with(Cell::get);
    let mut delivered = 0usize;
    read_track(
        &mut image,
        &toc,
        1,
        &ReadPlan::default(),
        &mut |s| {
            delivered += s.len();
            true
        },
        &|_| {},
    )
    .unwrap();
    let used = ALLOCATIONS.with(Cell::get) - before;
    assert_eq!(delivered, sectors as usize * 1176);
    used
}

#[test]
fn reading_more_sectors_allocates_no_more() {
    let short = allocations_to_read(30);
    let long = allocations_to_read(3000);
    assert_eq!(short, long, "allocations must not grow with the track length");
}
