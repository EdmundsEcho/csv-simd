use std::io;

use memmap::Mmap;

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use core::mem;

/// Load a 128-bit vector from slice at the given position. The slice does
/// not need to be unaligned.
///
#[target_feature(enable = "sse2")]
pub unsafe fn loadu128(slice: &[u8], at: usize) -> __m128i {
    let ptr = slice.get_unchecked(at..).as_ptr();
    _mm_loadu_si128(ptr as *const u8 as *const __m128i)
}

/// Returns a 128-bit vector with all bits set to 0.
#[target_feature(enable = "sse2")]
pub unsafe fn zeroes128() -> __m128i {
    _mm_set1_epi8(0)
}
