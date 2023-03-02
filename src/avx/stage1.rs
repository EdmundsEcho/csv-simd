#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use std::mem;

use crate::print_bitset_lookup;

pub const INPUT_LENGTH: usize = 4; // @128 mem alignment; 2 x 64-bit x 4 vectors

/// 64-byte input
#[derive(Debug)]
pub(crate) struct SimdInput {
    v0: __m128i,
    v1: __m128i,
    v2: __m128i,
    v3: __m128i,
}

impl SimdInput {
    #[cfg_attr(not(feature = "no-inline"), inline)]
    pub(crate) fn new(ptr: &[__m128]) -> Self {
        unsafe {
            Self {
                // Load 128-bits of integer data from memory into dst.
                // mem_addr must be aligned on a 16-byte boundary or a
                // general-protection exception may be generated.
                v0: _mm_load_si128(ptr.as_ptr() as *const __m128i),
                v1: _mm_load_si128(ptr.as_ptr().add(1) as *const __m128i),
                v2: _mm_load_si128(ptr.as_ptr().add(2) as *const __m128i),
                v3: _mm_load_si128(ptr.as_ptr().add(3) as *const __m128i),
            }
        }
    }
    /// padding in the number of 128 vectors
    pub(crate) fn new_with_padding(ptr: &[__m128], tail: &[u8]) -> Self {
        let load = ptr.len();
        println!("---------------------------------------------------------------------------------");
        println!("Last iteration");
        println!("Number of vectors to load {}", load);
        println!("The tail len: {}", tail.len());
        println!("---------------------------------------------------------------------------------\n");

        assert!(
            load < 4,
            "The SSE/AVX simdinput is not being initialized correctly"
        );
        assert!(
            tail.len() < 16,
            "The SSE/AVX tail_u8 is not being initialized correctly"
        );

        let mut padded_tail: [u8; 16] = [0; 16];
        tail.iter()
            .enumerate()
            .for_each(|(i, value_u8)| padded_tail[i] = *value_u8);

        println!("The now padded tail: {:?}", &padded_tail);

        unsafe {
            let padded_tail = padded_tail.get_unchecked(0..).as_ptr();

            match load {
                3 => Self {
                    v0: _mm_load_si128(ptr.as_ptr() as *const __m128i),
                    v1: _mm_load_si128(ptr.as_ptr().add(1) as *const __m128i),
                    v2: _mm_load_si128(ptr.as_ptr().add(2) as *const __m128i),
                    v3: _mm_loadu_si128(padded_tail as *const __m128i),
                },
                2 => Self {
                    v0: _mm_load_si128(ptr.as_ptr() as *const __m128i),
                    v1: _mm_load_si128(ptr.as_ptr().add(1) as *const __m128i),
                    v2: _mm_loadu_si128(padded_tail as *const __m128i),
                    v3: _mm_setzero_si128(),
                },
                1 => Self {
                    v0: _mm_load_si128(ptr.as_ptr() as *const __m128i),
                    v1: _mm_loadu_si128(padded_tail as *const __m128i),
                    v2: _mm_setzero_si128(),
                    v3: _mm_setzero_si128(),
                },
                0 => Self {
                    v0: _mm_loadu_si128(padded_tail as *const __m128i),
                    v1: _mm_setzero_si128(),
                    v2: _mm_setzero_si128(),
                    v3: _mm_setzero_si128(),
                },
                _ => panic!(
                    "The SSE/AVX simdinput is not being initialized correctly"
                ),
            }
        }
    }
}

use crate::high_nibble_mask;
use crate::low_nibble_mask;
use crate::stage1::Stage1;

#[macro_export]
macro_rules! set1_epi8 {
    ($mask:expr) => {
        _mm_set1_epi8($mask);
    };
}

/// return quote (16) or escape (8) or space (4)
/// 128-bit x 4 with 8-bit utf8 -> 64-bit
#[cfg_attr(not(feature = "no-inline"), inline(always))]
fn get_struct_positions(
    search: u8,
    res0: __m128i,
    res1: __m128i,
    res2: __m128i,
    res3: __m128i,
) -> u64 {
    unsafe {
        let struct_mask = set1_epi8!(search as i8);
        let struct0 = _mm_and_si128(res0, struct_mask);
        let struct1 = _mm_and_si128(res1, struct_mask);
        let struct2 = _mm_and_si128(res2, struct_mask);
        let struct3 = _mm_and_si128(res3, struct_mask);

        #[cfg(debug_assertions)]
        {
            // show result for v0-3
            println!("----------------------------------------------------------------------------------");
            println!("📋 structure v0-3: {}", search);
            let tmp = mem::transmute::<_, [u8; 16]>(struct0);
            println!("struct (v0):   {:?}", tmp);
            let tmp = mem::transmute::<_, [u8; 16]>(struct1);
            println!("struct (v1):   {:?}", tmp);
            let tmp = mem::transmute::<_, [u8; 16]>(struct2);
            println!("struct (v2):   {:?}", tmp);
            let tmp = mem::transmute::<_, [u8; 16]>(struct3);
            println!("struct (v3):   {:?}", tmp);
        }

        // let odd_mask: __m128i = _mm_set1_epi16(0x5555);
        // let even_mask: __m128i = _mm_cmpeq_epi32(_mm_setzero_si128(), odd_mask);
        let zero: __m128i = _mm_setzero_si128();
        //
        // This completes the 64-bytes -> 64-bits
        // 0xff | 0 (8-bits)
        // next, take the most significant to summarize the result
        //
        // 🔑 movmsk[b, ps, pd] r32, xmm
        //    movmskps eax
        //
        // 00000000_11111111 16-bytes
        // xxxxxxxxxxxxxxx01 16-bit
        //
        //
        let tmp_v0: __m128i = _mm_cmpeq_epi8(struct0, zero);
        let struct_res_0: u64 =
            u64::from(mem::transmute::<_, u32>(_mm_movemask_epi8(tmp_v0)));

        let tmp_v1: __m128i = _mm_cmpeq_epi8(struct1, zero);
        let struct_res_1: u64 =
            u64::from(mem::transmute::<_, u32>(_mm_movemask_epi8(tmp_v1)));

        let tmp_v2: __m128i = _mm_cmpeq_epi8(struct2, zero);
        let struct_res_2: u64 =
            u64::from(mem::transmute::<_, u32>(_mm_movemask_epi8(tmp_v2)));

        let tmp_v3: __m128i = _mm_cmpeq_epi8(struct3, zero);
        let struct_res_3: u64 =
            { u64::from(mem::transmute::<_, u32>(_mm_movemask_epi8(tmp_v3))) };

        //
        // 64-bytes -> 64-bits for escape (byte = 8 -> 0 | 1)
        //
        let positions = !(struct_res_0
            | (struct_res_1 << 16)
            | (struct_res_2 << 32)
            | (struct_res_3 << 48));

        // 16 x 8-bit -> 16 x 1-bit
        // Issue: the escapes live in u64
        //        epi64 operates on 128
        // Issue: shift left, we lose information required for the
        //        next vector to be processed.
        // let S = _mm_slli_epi64(mem::transmute::<_, u64>(escapes), 1);
        positions
    }
}
//
// Note: movemask and cmpgt might be under-utilized when comparing values.
//
impl Stage1<__m128i> for SimdInput {
    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn structure(&self, structure: &mut u64, in_string: &mut i64) {
        // the bit-set lookup tables for csv-related structure
        unsafe {
            // constant vectors
            let lo_nibble_mask: __m128i = low_nibble_mask!();
            let hi_nibble_mask: __m128i = high_nibble_mask!();

            #[cfg(debug_assertions)]
            {
                // 🧮 debug: extract 64-bits
                println!("🧮 v0-3 bit-set lookup of csv structure");
                let tmp00 = _mm_extract_epi64(self.v0, 0);
                let tmp00 = tmp00.to_le_bytes();
                let tmp00 =
                    std::str::from_utf8(&tmp00).expect("Fond invalid UTF-8");
                let tmp01 = _mm_extract_epi64(self.v0, 1);
                let tmp01 = tmp01.to_le_bytes();
                let tmp01 =
                    std::str::from_utf8(&tmp01).expect("Fond invalid UTF-8");

                let tmp10 = _mm_extract_epi64(self.v1, 0);
                let tmp10 = tmp10.to_le_bytes();
                let tmp10 =
                    std::str::from_utf8(&tmp10).expect("Fond invalid UTF-8");
                let tmp11 = _mm_extract_epi64(self.v1, 1);
                let tmp11 = tmp11.to_le_bytes();
                let tmp11 =
                    std::str::from_utf8(&tmp11).expect("Fond invalid UTF-8");

                let tmp20 = _mm_extract_epi64(self.v2, 0);
                let tmp20 = tmp20.to_le_bytes();
                let tmp20 =
                    std::str::from_utf8(&tmp20).expect("Fond invalid UTF-8");
                let tmp21 = _mm_extract_epi64(self.v2, 1);
                let tmp21 = tmp21.to_le_bytes();
                let tmp21 =
                    std::str::from_utf8(&tmp21).expect("Fond invalid UTF-8");

                let tmp30 = _mm_extract_epi64(self.v3, 0);
                let tmp30 = tmp30.to_le_bytes();
                let tmp30 =
                    std::str::from_utf8(&tmp30).expect("Fond invalid UTF-8");
                let tmp31 = _mm_extract_epi64(self.v3, 1);
                let tmp31 = tmp31.to_le_bytes();
                let tmp31 =
                    std::str::from_utf8(&tmp31).expect("Fond invalid UTF-8");
                println!(
                    "{}{}{}{}{}{}{}{}",
                    tmp00, tmp01, tmp10, tmp11, tmp20, tmp21, tmp30, tmp31
                );
            }

            // low nib = bitwise AND with low_mask
            // Ideal: Compute the bitwise AND of 256 bits (representing integer data) in a and b,
            // and store the result in dst.
            //
            let low_mask: __m128i = _mm_set1_epi8(0xf);
            // Compute the bitwise AND of elements in a and b,
            // store the results in dst.
            let nib_lo0 = _mm_and_si128(self.v0, low_mask);
            let nib_lo1 = _mm_and_si128(self.v1, low_mask);
            let nib_lo2 = _mm_and_si128(self.v2, low_mask);
            let nib_lo3 = _mm_and_si128(self.v3, low_mask);

            // build the hi nibble
            // Shift 4 to read the high nibble
            // shift right by the specified number of bits
            // store the results in dst.
            let nib_hi0 = _mm_srli_epi64(self.v0, 4);
            let nib_hi1 = _mm_srli_epi64(self.v1, 4);
            let nib_hi2 = _mm_srli_epi64(self.v2, 4);
            let nib_hi3 = _mm_srli_epi64(self.v3, 4);

            #[cfg(debug_assertions)]
            {
                // input
                println!("🚧 lo nibble (self.0, 0)");
                let tmp = _mm_extract_epi64(self.v0, 0);
                println!("v0 0 input:    {:#066b}", tmp);

                // mask
                let tmp = _mm_extract_epi64(low_mask, 0);
                println!("v0 0 mask:     {:#066b}", tmp);

                // input + low_mask
                let tmp = _mm_extract_epi64(nib_lo0, 0);
                println!("v0 0 nib_lo:   {:#066b}", tmp);

                println!("🚧 hi nibble");
                let tmp = _mm_extract_epi64(nib_hi0, 0);
                println!("v0 0 shift 4:  {:#066b}", tmp);
                println!("etc...");
            }

            // complete the shift with a mask
            let nib_hi0 = _mm_and_si128(nib_hi0, low_mask);
            let nib_hi1 = _mm_and_si128(nib_hi1, low_mask);
            let nib_hi2 = _mm_and_si128(nib_hi2, low_mask);
            let nib_hi3 = _mm_and_si128(nib_hi3, low_mask);

            // lookup
            // This is a SSE3 instruction
            // It is special in that implements runtime-variable shuffles:
            // The last 4 bits of each byte of b are used as addresses into the 16 bytes of a.
            // ... low nibble -> byte value (vec: lookup 16 times)
            // 🔖 AVX2 that Lemir uses: vpshufb _mm256_shuffle_epi8
            let shuf_lo0 = _mm_shuffle_epi8(lo_nibble_mask, nib_lo0);
            let shuf_lo1 = _mm_shuffle_epi8(lo_nibble_mask, nib_lo1);
            let shuf_lo2 = _mm_shuffle_epi8(lo_nibble_mask, nib_lo2);
            let shuf_lo3 = _mm_shuffle_epi8(lo_nibble_mask, nib_lo3);

            // ... high nibble -> byte value
            let shuf_hi0 = _mm_shuffle_epi8(hi_nibble_mask, nib_hi0);
            let shuf_hi1 = _mm_shuffle_epi8(hi_nibble_mask, nib_hi1);
            let shuf_hi2 = _mm_shuffle_epi8(hi_nibble_mask, nib_hi2);
            let shuf_hi3 = _mm_shuffle_epi8(hi_nibble_mask, nib_hi3);

            // 🎉 - all '\r', '\n', ',', ' ', '\', '"'
            // 16 x 8-bit (128)
            // combine lo/hi
            let res0 = _mm_and_si128(shuf_lo0, shuf_hi0);
            let res1 = _mm_and_si128(shuf_lo1, shuf_hi1);
            let res2 = _mm_and_si128(shuf_lo2, shuf_hi2);
            let res3 = _mm_and_si128(shuf_lo3, shuf_hi3);

            #[cfg(debug_assertions)]
            {
                // show all structure for v0-3
                println!("----------------------------------------------------------------------------------");
                println!("📋 all tokens v0-3");
                // show results of the lookup in v0 (2 x 64)
                let tmp = mem::transmute::<_, [u8; 16]>(res0);
                println!("result (v0):   {:?}", tmp);
                // show results of the lookup in v1 (2 x 64)
                let tmp = mem::transmute::<_, [u8; 16]>(res1);
                println!("result (v1):   {:?}", tmp);
                // show results of the lookup in v2 (2 x 64)
                let tmp = mem::transmute::<_, [u8; 16]>(res2);
                println!("result (v2):   {:?}", tmp);
                // show results of the lookup in v3 (2 x 64)
                let tmp = mem::transmute::<_, [u8; 16]>(res3);
                println!("result (v3):   {:?}", tmp);
            }
            // ** END GETTING TOKENS **
            let zero: __m128i = _mm_setzero_si128();
            let ones: __m128i = _mm_cmpeq_epi32(zero, zero);

            // 3 sub-routines that return __m128i
            // exploit the closure context to read parameters accordingly
            let string_mask_go = |quote_bits: u64| -> __m128i {
                //
                // tag start and stop
                // _111____1111 Backslash, B
                // 1_1_1_1_1_1_ Even
                // _1_1_1_1_1_1 Odd
                // _1______1___ B &~(B << 1)
                //
                // Goal:
                // 0b100010000 quotes
                // 0b011110000 string mask
                //
                // accomplished with clmul
                //
                // load the quote_bits into the first of two 64-bit slots
                // (zero into the other)
                let quote_bits: __m128i =
                    _mm_set_epi64x(0, mem::transmute::<_, i64>(quote_bits));
                _mm_clmulepi64_si128(quote_bits, ones, 0)
            };
            let not = |x: __m128i| -> __m128i { _mm_xor_si128(x, ones) };
            let in_str_flip = |x: __m128i| -> __m128i {
                // Task
                // This needs to occur at the end of each 64-byte -> 64-bit iteration.
                //
                // The string_mask needs flipping when the previous vector most significant bit
                // is value zero (i.e., ends inside a quote).
                // xor (x, 1) -> not
                // xor (x, 0) -> identity
                //
                // x  flip on/off
                // 0,           0 -> 0   identity
                // 1,           0 -> 1   identity
                // 0,           1 -> 1   flip
                // 1,           1 -> 0   flip
                //
                _mm_xor_si128(
                    x,
                    _mm_set_epi64x(0, mem::transmute::<_, i64>(*in_string)),
                )
            };

            // 1. find the quote bits; return u64 (16)
            // 2. apply string_mask_go(quote_bits); return __m128i
            // 3. String = apply in_str_flip to that; return _m128i
            // 4. Struct = find all struck (3)
            // 5. Result = Struct AND ~String
            // 6.
            //
            // quote      0b00010000  (16)
            let quote_bits = get_struct_positions(16, res0, res1, res2, res3);
            // comman or return      0b00000011  (3)
            let all_struct = get_struct_positions(3, res0, res1, res2, res3);

            // use the in_string set in the previous iteration
            let string_mask: __m128i = in_str_flip(string_mask_go(quote_bits));

            // the masked structure
            let result = _mm_and_si128(
                _mm_set_epi64x(0, mem::transmute::<_, i64>(all_struct)), // load 64i into 1 | 0 register
                not(string_mask),
            );

            // extract, and set
            *structure = _mm_cvtsi128_si64(result) as u64;
            *in_string = _mm_cvtsi128_si64(string_mask) as i64 >> 63;

            #[cfg(debug_assertions)]
            {
                println!("----------------------------------------------------------------------------------");
                println!("👉 structure result WIP");
                println!("----------------------------------------------------------------------------------");

                self.show();

                println!("all struct:    {:#066b}", all_struct);
                println!("quotes:        {:#066b}", quote_bits);
                println!(
                    "string mask    {:#066b}",
                    _mm_cvtsi128_si64(string_mask) as i64
                );
                println!("in_str_next    {:#066b}", in_string);
                println!("result:        {:#066b}", &structure);

                print_bitset_lookup!();
                println!("-------------------");
            }
        }
    }
    /// Display the string representation of the 4 vectors of bytes
    /// ... for debugging purposes only.
    fn show(&self) {
        unsafe {
            // show the string representation the data
            let tmp00 = _mm_extract_epi64(self.v0, 0);
            let tmp00 = tmp00.to_le_bytes();
            let tmp00 =
                std::str::from_utf8(&tmp00).expect("Found invalid UTF-8");
            let tmp01 = _mm_extract_epi64(self.v0, 1);
            let tmp01 = tmp01.to_le_bytes();
            let tmp01 =
                std::str::from_utf8(&tmp01).expect("Found invalid UTF-8");

            let tmp10 = _mm_extract_epi64(self.v1, 0);
            let tmp10 = tmp10.to_le_bytes();
            let tmp10 =
                std::str::from_utf8(&tmp10).expect("Found invalid UTF-8");
            let tmp11 = _mm_extract_epi64(self.v1, 1);
            let tmp11 = tmp11.to_le_bytes();
            let tmp11 =
                std::str::from_utf8(&tmp11).expect("Found invalid UTF-8");

            let tmp20 = _mm_extract_epi64(self.v2, 0);
            let tmp20 = tmp20.to_le_bytes();
            let tmp20 =
                std::str::from_utf8(&tmp20).expect("Found invalid UTF-8");
            let tmp21 = _mm_extract_epi64(self.v2, 1);
            let tmp21 = tmp21.to_le_bytes();
            let tmp21 =
                std::str::from_utf8(&tmp21).expect("Found invalid UTF-8");

            let tmp30 = _mm_extract_epi64(self.v3, 0);
            let tmp30 = tmp30.to_le_bytes();
            let tmp30 =
                std::str::from_utf8(&tmp30).expect("Found invalid UTF-8");
            let tmp31 = _mm_extract_epi64(self.v3, 1);
            let tmp31 = tmp31.to_le_bytes();
            let tmp31 =
                std::str::from_utf8(&tmp31).expect("Found invalid UTF-8");

            println!(
                "{}{}{}{}{}{}{}{}",
                tmp00, tmp01, tmp10, tmp11, tmp20, tmp21, tmp30, tmp31
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
