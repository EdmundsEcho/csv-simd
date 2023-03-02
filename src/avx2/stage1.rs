#![allow(dead_code)]
// use crate::helper::ByteReport;
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use std::mem;

macro_rules! low_nibble_mask {
    () => {
        _mm256_setr_epi8(
            4, 0, 16, 0, 0, 0, 0, 0, 0, 0, 1, 0, 10, 1, 0, 0, 4, 0, 16, 0, 0, 0, 0, 0, 0, 0, 1, 0,
            10, 1, 0, 0,
        )
    };
}

macro_rules! high_nibble_mask {
    () => {
        _mm256_setr_epi8(
            1, 0, 22, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 22, 0, 0, 8, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        )
    };
}
pub const INPUT_PADDING: usize = mem::size_of::<__m256i>();
pub const INPUT_LENGTH: usize = 64;

#[derive(Debug)]
pub(crate) struct SimdInput {
    v0: __m256i,
    v1: __m256i,
}

impl SimdInput {
    #[cfg_attr(not(feature = "no-inline"), inline)]
    pub(crate) fn new(ptr: &[__m128]) -> Self {
        unsafe {
            Self {
                // Load 256-bits of integer data from memory into dst.
                // mem_addr must be aligned on a 32-byte boundary or a
                // general-protection exception may be generated.
                v0: _mm256_load_si256(ptr.as_ptr() as *const __m256i),
                v1: _mm256_load_si256(ptr.as_ptr().add(2) as *const __m256i),
            }
        }
    }
}

use crate::Stage1;

impl Stage1<__m256i> for SimdInput {
    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    #[allow(clippy::cast_sign_loss)]
    fn structure(&self, structure: &mut u64, debug: bool) {
        // the bit-set lookup tables for csv-related structure
        unsafe {
            // the three constants
            let lo_nibble_mask: __m256i = low_nibble_mask!();
            let hi_nibble_mask: __m256i = high_nibble_mask!();

            // Broadcast 8-bit integer a to all elements of dst.
            // 0b00001111 x 16
            let low_mask: __m256i = _mm256_set1_epi8(0xf);

            // Haystack
            // input 16 x 16-bit
            /*
            let buff = std::str::from_utf8(chunk).expect("Found invalid UTF-8");
            println!("buff {:?}", &buff);
            println!("chunk {:?}", &chunk);
            */

            // low nib = bitwise AND with low_mask
            // Compute the bitwise AND of 128 bits (representing integer data) in a and b,
            // and store the result in dst.
            let nib_lo: __m256i = _mm256_and_si256(self.v0, low_mask);

            /*
             *  🚨 Runtime - Fix by detecting supported instructions
             *
            // high nib = shift 4, apply low_mask
            let nib_hi = _mm256_srl_epi64(self.v0, 4);
            let nib_hi = _mm256_and_si256(nib_hi, low_mask);
            // let temp = mem::transmute::<_, [u8; 16]>(nib_hi);
            println!("Here");
            dbg!(&nib_hi);

            // lookup
            // The last 4 bits of each byte of b are used as addresses into the 16 bytes of a.
            // ... low nibble -> byte value (vec: lookup 16 times)
            let shuf_lo = _mm256_shuffle_epi8(lo_nibble_mask, nib_lo);
            // ... high nibble -> byte value
            let shuf_hi = _mm256_shuffle_epi8(hi_nibble_mask, nib_hi);

            // 🎉
            // combine lo/hi
            let res0 = _mm256_and_si256(shuf_lo, shuf_hi);
            let res0: u64 = u64::from(mem::transmute::<_, u32>(_mm256_movemask_epi8(res0)));

            // Repeat for the second SimdInput vector
            let nib_lo: __m256i = _mm256_and_si256(self.v0, low_mask);
            let nib_hi = _mm256_srli_epi64(self.v0, 4);
            let nib_hi = _mm256_and_si256(nib_hi, low_mask);
            let shuf_lo = _mm256_shuffle_epi8(lo_nibble_mask, nib_lo);
            let shuf_hi = _mm256_shuffle_epi8(hi_nibble_mask, nib_hi);

            // 🎉
            let res1 = _mm256_and_si256(shuf_lo, shuf_hi);
            let res1: u64 = _mm256_movemask_epi8(res1) as u64;

            *structure = !(res0 | (res1 << 4));
            */
        }
    }
    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    #[allow(clippy::cast_possible_wrap, clippy::cast_ptr_alignment)]
    fn crush_set_bits(
        base: &mut Vec<usize>,
        mut set_bits: u64,
        codepoint_cnt: usize,
        array_idx: &mut u32,
    ) {
        let mut buff =
            unsafe { std::mem::transmute::<_, [u32; 8]>([_mm_setzero_ps(), _mm_setzero_ps()]) };
        let b = &mut buff;

        // let &mut b = buff;
        let cnt = set_bits.count_ones();
        let cursor = base.len() as u32;
        let next_base = cursor + cnt;

        // expand the Vector size
        // base.reserve(8);
        //
        // create a place for the vec result:
        //
        // __m128i_mm_setzero_si128()                 ; returns 128-bit zero vector
        //
        // __128i_mm_loadu_si128(__m128i*p)           ; load data stored at p of memory to a 128bit vector
        //                                            ; return this vector
        //
        // __128i_mm_add_epi32(__m128i a, __m128i b)  ; return vector(a0+b0, a1+b1 etc n = 4)
        // void_mm_storeu_si128(__m128i*p, __m128i a) ; store content off 128-bit vec "a" ato mem
        //                                            ; starting at pointer p
        //
        // __m128i temp  = _mm_setzero_si128();
        // __m128i temp1 = _mm_loadu_si128((__m128i*)(a+1));
        // temp = _mm_add_epi32(temp, temp1);
        //
        b[0] = cursor + set_bits.trailing_zeros();
        set_bits = set_bits & set_bits.saturating_sub(1); // clear away the lowest set bit
        b[1] = cursor + set_bits.trailing_zeros();
        set_bits = set_bits & set_bits.saturating_sub(1);
        b[2] = cursor + set_bits.trailing_zeros();
        set_bits = set_bits & set_bits.saturating_sub(1);
        b[3] = cursor + set_bits.trailing_zeros();
        set_bits = set_bits & set_bits.saturating_sub(1);
        b[4] = cursor + set_bits.trailing_zeros();
        set_bits = set_bits & set_bits.saturating_sub(1);
        b[5] = cursor + set_bits.trailing_zeros();
        set_bits = set_bits & set_bits.saturating_sub(1);
        b[6] = cursor + set_bits.trailing_zeros();
        set_bits = set_bits & set_bits.saturating_sub(1);
        b[7] = cursor + set_bits.trailing_zeros();
        set_bits = set_bits & set_bits.saturating_sub(1);

        /*
        // load the result...why?
        // __128i_mm_loadu_si128(__m128i*p); load data stored at p of memory to a 128bit vector

        // store the result
        // Option 1: extend memory by 8 every time *but* from the previous computed base
        //           next_base = base + cnt;
        //
        // Option 2: extend by cnt
        //
        base.extend_from_slice(&b[..cnt as usize]);
        // let chunk = std::ptr::write_volatile(base, *b);

        // 💡 How about always writing 8
        //    base---[00000000]
        //                ^ start mem pointer?
        // let idx_minus_64 = b.wrapping_sub(64);
        // write to memory

        println!("set_bits (consumed): {:?}", &set_bits);
        println!("next_base: {:?}", &next_base);
        println!("base: {:?}", base);

        // set b = new_base
        */
    }
}
