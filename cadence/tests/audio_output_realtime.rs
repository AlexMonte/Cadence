//! Callback allocation audit. The allocator only measures this test thread;
//! work performed by the render worker is deliberately outside the callback.
#![cfg(not(target_arch = "wasm32"))]

use cadence::adapter::audio::{AudioRenderer, AudioRendererSettings, output::start_output_worker};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
struct CallbackAuditAllocator;
// SAFETY: every allocation operation delegates to System with its original
// pointer and layout. Thread-local counters neither allocate nor share state.
unsafe impl GlobalAlloc for CallbackAuditAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        MEASURING.with(|active| {
            if active.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        MEASURING.with(|active| {
            if active.get() {
                DEALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe {
            System.dealloc(ptr, layout);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        MEASURING.with(|active| {
            if active.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CallbackAuditAllocator = CallbackAuditAllocator;

#[test]
fn callback_allocates_and_reclaims_nothing_including_underrun_and_cancel() {
    let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(48_000, 64)).unwrap();
    let (mut output, _worker) = start_output_worker(renderer, 960).unwrap();
    // Wait outside the measured callback to establish normal output availability.
    std::thread::sleep(std::time::Duration::from_millis(5));
    ALLOCATIONS.with(|count| count.set(0));
    DEALLOCATIONS.with(|count| count.set(0));
    MEASURING.with(|active| active.set(true));
    for _ in 0..1024 {
        std::hint::black_box(output.next_frame());
    }
    audio.cancel_all();
    for _ in 0..1024 {
        std::hint::black_box(output.next_frame());
    }
    audio.panic();
    for _ in 0..1024 {
        std::hint::black_box(output.next_frame());
    }
    MEASURING.with(|active| active.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0, "device callback allocated");
    assert_eq!(
        DEALLOCATIONS.with(Cell::get),
        0,
        "device callback reclaimed memory"
    );
}
