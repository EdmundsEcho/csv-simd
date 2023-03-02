/// Deprecated
///
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use core::mem;

pub fn run(chunk: &[u8], at: usize) -> [u8; 16] {
    // the bit-set lookup tables for csv-related structure
    unsafe {
        macro_rules! low_nibble_mask {
            () => {
                _mm_setr_epi8(4, 0, 16, 0, 0, 0, 0, 0, 0, 0, 1, 0, 10, 1, 0, 0)
            };
        }

        macro_rules! high_nibble_mask {
            () => {
                _mm_setr_epi8(1, 0, 22, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
            };
        }

        // the three constants
        let lo_nibble_mask: __m128i = low_nibble_mask!();
        let hi_nibble_mask: __m128i = high_nibble_mask!();

        // Broadcast 8-bit integer a to all elements of dst.
        // 0b00001111 x 16
        let low_mask: __m128i = _mm_set1_epi8(0xf);

        // load the chunk into an mm vector
        let ptr = chunk.get_unchecked(at..).as_ptr();
        let chunk = _mm_loadu_si128(ptr as *const __m128i);

        // low nib = bitwise AND with low_mask
        // Compute the bitwise AND of 128 bits (representing integer data)
        // in a and b, and store the result in dst.
        let nib_lo: __m128i = _mm_and_si128(chunk, low_mask);

        // high nib = shift 4, apply low_mask
        let nib_hi = _mm_srli_epi64(mem::transmute(chunk), 4);
        let nib_hi = _mm_and_si128(nib_hi, low_mask);

        // lookup
        // The last 4 bits of each byte of b are used as addresses into the 16 bytes of a.
        // ... low nibble -> byte value (vec: lookup 16 times)
        let shuf_lo = _mm_shuffle_epi8(lo_nibble_mask, nib_lo);
        // ... high nibble -> byte value
        let shuf_hi = _mm_shuffle_epi8(hi_nibble_mask, nib_hi);

        // combine lo/hi
        let result = _mm_and_si128(shuf_lo, shuf_hi);

        mem::transmute::<_, [u8; 16]>(result)
    }
}

pub unsafe fn loadu128(slice: &[u8], at: usize) -> __m128i {
    let ptr = slice.get_unchecked(at..).as_ptr();
    _mm_loadu_si128(ptr as *const u8 as *const __m128i)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn structure() {
        // given...
        let mask: u8 = 0xf;
        let win: u8 = 123;
        let low = (mask & win) as usize;
        assert_eq!(low, 0xb as usize, "Isolated test");
        assert_eq!(low, 11 as usize, "Isolated test");
    }
}
