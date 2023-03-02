///
///  For data analysis:
///
///     👉 FIX: fixed number of fields
///
///     👉 FIX: no escape, no comma inside quotes (For Now)
///
///     👉 data structure for storage:
///        * 1-D array
///          * index = field count
///            * record number = round down [field count] / [number of fields]
///            * field number  = [field count] mod [number of fields]
///          * value = io::offset
///
use std::arch::x86_64::*;
use std::mem;

/// default bit-count size
/// (for a given bit-set, the bit-count tags whether the 16-bit value is a member of the set)
/// 64-bytes -> 64-bits
const BUFF_EXTENSION: u8 = 64;

#[macro_export]
macro_rules! low_nibble_mask {
    () => {
        _mm_setr_epi8(4, 0, 16, 0, 0, 0, 0, 0, 0, 0, 1, 0, 10, 1, 0, 0)
    };
}

#[macro_export]
macro_rules! high_nibble_mask {
    () => {
        _mm_setr_epi8(1, 0, 22, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
    };
}

/// Report the bit set lookup table used to identify structure-related 16-bit UTF code-points.
#[macro_export]
macro_rules! print_bitset_lookup {
    () => {
        let output = r#"
    Structure  UTF8   Code
    ----------------------
    newline    d|a       1
    comma       2c       2
    space       20       4
    escape      5c       8
    quote       22      16

    mask for structure: 0b11
    todo: trim " xx ",

    "#;
        println!("{}", output);
    };
}

/// The representation of the csv structure. The index value is the offset in code-units for UTF8.
/// The code-point values represent record and field delimiters.
#[derive(Debug)]
pub struct StructureIndex(pub Vec<CodeUnitPos>);

/// The memory offset position of a code-unit. The collection of these values is hosted in the
/// `StructureIndex`. The min and max values must fall within the range of the memory hosting Data.
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(transparent)]
pub struct CodeUnitPos(usize);
unsafe impl bytemuck::Pod for CodeUnitPos {}
unsafe impl bytemuck::Zeroable for CodeUnitPos {}

/// The lookup-key to retrieve a CodeUnitPos. The Chunks will utilize these values.
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(transparent)]
pub struct KeyToPos(pub usize);
unsafe impl bytemuck::Pod for KeyToPos {}
unsafe impl bytemuck::Zeroable for KeyToPos {}

// ------------------------------------------------------------------------------
// CodeUnitPos trait implementations
// 👎 using Deref to effect T -> U is an anti-pattern
//    Deref is intended to ptr T -> T
//    Deref does not work on values (only works on refs)
//    Deref does not work with generics
// ------------------------------------------------------------------------------
impl std::fmt::Display for CodeUnitPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::ops::Deref for CodeUnitPos {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        // & of self . & of Wrap -> & of usize
        &self.0
    }
}
impl std::ops::DerefMut for CodeUnitPos {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// ------------------------------------------------------------------------------
// KeyToPos trait implementations
// ------------------------------------------------------------------------------
impl std::fmt::Display for KeyToPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::ops::Deref for KeyToPos {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        // & of self . & of Wrap -> & of usize
        &self.0
    }
}
impl std::ops::DerefMut for KeyToPos {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
// ------------------------------------------------------------------------------
// StructureIndex trait implementations
// ------------------------------------------------------------------------------
impl std::fmt::Display for StructureIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.len();
        write!(
            f,
            "StructureIndex len: {} first: {} last: {}",
            len,
            self.0[0],
            self.0[len - 1]
        )
    }
}

impl std::ops::Deref for StructureIndex {
    type Target = Vec<CodeUnitPos>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for StructureIndex {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
// ------------------------------------------------------------------------------

/// Trait interface for processing the first csv processing stage.
pub(crate) trait Stage1<T> {
    fn structure(&self, structure: &mut u64, in_str: &mut i64);
    fn show(&self);
    /// Decode the set of bits from set_bits to the acc array
    /// (64-bits -> array with len + ??)
    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn crush_set_bits(
        acc: &mut Vec<usize>,
        mut set_bits: u64,
        codepoint_cnt: usize,
        array_idx: &mut u32, // a property of the acc (set_len)
    ) {
        //
        // Where set the limit to memory that points to a structure element.
        // The number of structural code points in a vector
        // intrinsic: _popcnt64 (__int64)
        //
        let cnt = set_bits.count_ones();

        // ~ C pointer but instead using index count
        let base = *array_idx as usize;
        let next_base = *array_idx as usize + cnt as usize;

        // codepoint_cnt ~ position in memory
        //
        // 🔑 codepoint_cnt advances faster than next_base
        //    True, because when no structure, the codepoint_cnt will advance,
        //    but not the next_base.

        // task
        // consume set_bits by removing the least set bet with every iteration
        #[cfg(debug_assertions)]
        {
            println!("Computing the index...");
            println!("Start value:   {:#066b}", set_bits);
            println!("base: {} next_base: {}", base, next_base);
        }

        //
        // increase the size of the acc by the max possible structure items
        // The "extra" / wasted is generated by what we don't use in the
        // fixed steps in the following loop.
        //
        // case: use less than reserved
        // We over-write what we don't use by adjusting the position of the
        // cursor (i.e., advance only by what we used).
        //
        // case: no structural elements exist
        // The record_item_count = zero.  The min iteration will set the memory
        // to zero.  The next call to the function will over-write the memory
        // with a cursor that is just that much further advanced.
        //
        // Record the current length (Note: tmp, next use array_idx)
        //

        unsafe {
            // increase the size of the collection to the theoretical max
            // number of slots required to tag the 64-bytes
            acc.reserve(BUFF_EXTENSION as usize);
            let ptr = acc.as_mut_ptr();
            acc.set_len(base as usize + BUFF_EXTENSION as usize);

            // point to the beginning of the collection

            // dynamics
            // 👉 advances when set_bits > 0 by the count of set bits
            // 👉 set_bits records cumulative changes: set_bits.saturating_sub(1)
            // 👉 the number of trailing_zeros increases every time
            // 🔑 the offset (index value) = fixed cursor + number of trailing zeros
            // 🔑 the index = cursor + 0, + 1, etc...
            //
            let mut shift = 0;
            // consume the set_bits to zero
            while set_bits != 0 {
                #[cfg(debug_assertions)]
                println!("codepoint value: {:?}", codepoint_cnt);
                // count leading zeros
                *ptr.add(base + 0 + shift) =
                    codepoint_cnt + set_bits.trailing_zeros() as usize;
                #[cfg(debug_assertions)]
                println!("set_bits zero: {:#066b}", set_bits);
                // generate the next value of set_bits (bringing it closer to zero)
                // clear away the lowest set bit remove least set bit
                set_bits &= set_bits.saturating_sub(1);

                *ptr.add(base + 1 + shift) =
                    codepoint_cnt + set_bits.trailing_zeros() as usize;
                #[cfg(debug_assertions)]
                println!("set_bits zero: {:#066b}", set_bits);
                set_bits &= set_bits.saturating_sub(1);

                *ptr.add(base + 2 + shift) =
                    codepoint_cnt + set_bits.trailing_zeros() as usize;
                #[cfg(debug_assertions)]
                println!("set_bits zero: {:#066b}", set_bits);
                set_bits &= set_bits.saturating_sub(1);

                *ptr.add(base + 3 + shift) =
                    codepoint_cnt + set_bits.trailing_zeros() as usize;
                #[cfg(debug_assertions)]
                println!("set_bits zero: {:#066b}", set_bits);
                set_bits &= set_bits.saturating_sub(1);

                *ptr.add(base + 4 + shift) =
                    codepoint_cnt + set_bits.trailing_zeros() as usize;
                #[cfg(debug_assertions)]
                println!("set_bits zero: {:#066b}", set_bits);
                set_bits &= set_bits.saturating_sub(1);

                *ptr.add(base + 5 + shift) =
                    codepoint_cnt + set_bits.trailing_zeros() as usize;
                #[cfg(debug_assertions)]
                println!("set_bits zero: {:#066b}", set_bits);
                set_bits &= set_bits.saturating_sub(1);

                *ptr.add(base + 6 + shift) =
                    codepoint_cnt + set_bits.trailing_zeros() as usize;
                #[cfg(debug_assertions)]
                println!("set_bits zero: {:#066b}", set_bits);
                set_bits &= set_bits.saturating_sub(1);

                *ptr.add(base + 7 + shift) =
                    codepoint_cnt + set_bits.trailing_zeros() as usize;
                #[cfg(debug_assertions)]
                println!("set_bits zero: {:#066b}", set_bits);
                set_bits &= set_bits.saturating_sub(1);

                // report the acc
                #[cfg(debug_assertions)]
                println!("acc: {:?}", acc);
                #[cfg(debug_assertions)]
                println!("next base: {:?}", next_base);
                *array_idx = *array_idx + 8;
                shift = shift + 8;
            }
            acc.set_len(next_base);
            *array_idx = next_base as u32;
            #[cfg(debug_assertions)]
            println!("acc len: {:?}", acc.len());
        }
    }
}

/// return quote (16) or escape (8) or space (4)
/// 128-bit x 4 with 8-bit utf8 -> 64-bit
#[cfg_attr(not(feature = "no-inline"), inline(always))]
fn get_struct_positions(search: u8, res0: __m128i) -> u64 {
    unsafe {
        // let struct_mask = set1_epi8!(search as i8);
        let struct_mask: __m128i = _mm_set1_epi8(search as i8);
        let struc = _mm_and_si128(res0, struct_mask);

        #[cfg(debug_assertions)]
        {
            // show result for the single vector
            println!("----------------------------------------------------------------------------------");
            println!("📋 structure vTail: {}", search);
            let tmp = mem::transmute::<_, [u8; 16]>(struc);
            println!("struct (v0):   {:?}", tmp);
        }

        let zero: __m128i = _mm_setzero_si128();

        let tmp: __m128i = _mm_cmpeq_epi8(struc, zero);

        let tmp: i32 = !(_mm_movemask_epi8(tmp));
        let tmp = (tmp << 16) >> 16;
        let tmp: u64 = u64::from(mem::transmute::<_, u32>(tmp));

        // let positions = tmp | (0 << 48);

        tmp
    }
}
/// 64-byte input
#[derive(Debug)]
pub(crate) struct SimdInputFragment {
    v: __m128i,
}

impl SimdInputFragment {
    #[cfg_attr(not(feature = "no-inline"), inline)]
    pub(crate) fn new(ptr: &[u8]) -> Self {
        unsafe {
            // load the chunk into an mm vector
            let ptr = ptr.get_unchecked(0..).as_ptr();
            Self {
                v: _mm_loadu_si128(ptr as *const __m128i),
            }
        }
    }
}

impl Stage1<__m128i> for SimdInputFragment {
    fn structure(&self, structure: &mut u64, in_string: &mut i64) {
        //
        // ⬜ Make the structure u64 generic; where we need u16 or i32
        //
        unsafe {
            // constant vectors
            let lo_nibble_mask: __m128i = low_nibble_mask!();
            let hi_nibble_mask: __m128i = high_nibble_mask!();
            let low_mask: __m128i = _mm_set1_epi8(0xf);
            // let zero: __m128i = _mm_set1_epi8(0x0); //0b11

            // low nib = bitwise AND with low_mask
            // Compute the bitwise AND of 128 bits (representing integer data)
            // in a and b, and store the result in dst.
            let nib_lo: __m128i = _mm_and_si128(self.v, low_mask);

            // high nib = shift 4, apply low_mask
            let nib_hi = _mm_srli_epi64(mem::transmute(self.v), 4);
            let nib_hi = _mm_and_si128(nib_hi, low_mask);

            // lookup
            // The last 4 bits of each byte of b are used as addresses into the 16 bytes of a.
            // ... low nibble -> byte value (vec: lookup 16 times)
            let shuf_lo = _mm_shuffle_epi8(lo_nibble_mask, nib_lo);
            // ... high nibble -> byte value
            let shuf_hi = _mm_shuffle_epi8(hi_nibble_mask, nib_hi);

            // combine lo/hi
            // all structure in the lookup
            let res0 = _mm_and_si128(shuf_lo, shuf_hi);

            // ** END GETTING TOKENS **
            let zero: __m128i = _mm_setzero_si128();
            let ones: __m128i = _mm_cmpeq_epi32(zero, zero);

            // 3 sub-routines that return __m128i
            let string_mask_go = |quote_bits: u64| -> __m128i {
                let quote_bits: __m128i =
                    _mm_set_epi64x(0, mem::transmute::<_, i64>(quote_bits));
                _mm_clmulepi64_si128(quote_bits, ones, 0)
            };

            let not = |x: __m128i| -> __m128i { _mm_xor_si128(x, ones) };
            let in_str_flip = |x: __m128i| -> __m128i {
                _mm_xor_si128(
                    x,
                    _mm_set_epi64x(0, mem::transmute::<_, i64>(*in_string)),
                )
            };
            // END sub-routines

            // 1. find the quote bits; return u64 (16)
            // 2. apply string_mask_go(quote_bits); return __m128i
            // 3. String = apply in_str_flip to that; return _m128i
            // 4. Struct = find all struck (3)
            // 5. Result = Struct AND ~String
            // 6.
            //
            // quote      0b00010000  (16)
            let quote_bits = get_struct_positions(16, res0);
            // comman or return      0b00000011  (3)
            let all_struct = get_struct_positions(3, res0);

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

            // ⚠️  only the first 16-bits are relevant
            #[cfg(debug_assertions)]
            {
                println!("----------------------------------------------------------------------------------");
                println!("🦀 structure result WIP");
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
            let tmp00 = _mm_extract_epi64(self.v, 0);
            let tmp00 = tmp00.to_le_bytes();
            let tmp00 =
                std::str::from_utf8(&tmp00).expect("Fond invalid UTF-8");
            let tmp01 = _mm_extract_epi64(self.v, 1);
            let tmp01 = tmp01.to_le_bytes();
            let tmp01 =
                std::str::from_utf8(&tmp01).expect("Fond invalid UTF-8");

            println!("{}{}", tmp00, tmp01);
        }
    }
}
/// LineEnding is an alternative name.  The approach used by Rust is the search for \n, then remove
/// the \r at the end of each line.
///
#[derive(Debug)]
#[non_exhaustive]
pub enum NewLine {
    /// move down to the next line; move to the beginning
    CRLF, // end of line: CR and LF, \r\n, 0x0d0a
    /// move down to the next line; no move to the beginning
    LF, // line feed: LF, \n, 0x0a
    /// placeholder for another u8 encoding
    Any(u8),
    // CR, // move to the beginning of the current line
}
impl NewLine {
    fn is_crlf(&self) -> bool {
        match *self {
            NewLine::CRLF => true,
            NewLine::Any(_) => false,
            _ => unreachable!(),
        }
    }
    fn equals(&self, other: u8) -> bool {
        match *self {
            NewLine::CRLF => other == b'\r' || other == b'\n',
            NewLine::Any(b) => other == b,
            _ => unreachable!(),
        }
    }
}

impl Default for NewLine {
    fn default() -> NewLine {
        NewLine::CRLF
    }
}
