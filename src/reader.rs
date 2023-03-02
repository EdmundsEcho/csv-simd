///
/// Terminology
///
/// code-point: Is a unit of information that when combined with others
///             encode the information in a text.
///
/// code-unit: The underlying computer bits required to record the unit of information.
///            e.g., UTF8 code-points are recorded using 8-bit code-units; UTF16 code-points
///            are encoded using the 16-bit code-unit. The number of code-units used for
///            each code-point *can vary*, but is normally one.
///
use core::arch::x86_64::*;
use std::mem;

use bytemuck::allocation::cast_vec;
use memmap::Mmap;

use crate::avx::stage1::SimdInput;
use crate::avx::stage1::INPUT_LENGTH;
use crate::helper::ByteReport;
use crate::stage1::StructureIndex;
use crate::stage1::{SimdInputFragment, Stage1};
// use crate::reader;

/// Non-core
/// Returns `true` if any byte in the word `v` is nonascii (>= 128). Snarfed
/// from `../str/mod.rs`, which does something similar for utf8 validation.
#[inline]
fn contains_nonascii(v: usize) -> bool {
    const NONASCII_MASK: usize = 0x80808080_80808080u64 as usize;
    (NONASCII_MASK & v) != 0
}

/// Non-core
#[inline]
pub fn is_ascii(s: &[u8]) -> bool {
    const USIZE_SIZE: usize = mem::size_of::<usize>();
    #[cfg(debug)]
    println!("mem size in bytes of usize: {:?}", &USIZE_SIZE);

    let len = s.len();
    #[cfg(debug)]
    println!("len of the [u8]: {:?}", &len);

    let align_offset = s.as_ptr().align_offset(USIZE_SIZE);
    #[cfg(debug)]
    println!("offset: {:?}", &align_offset);

    // If we wouldn't gain anything from the word-at-a-time implementation, fall
    // back to a scalar loop.
    //
    // We also do this for architectures where `size_of::<usize>()` isn't
    // sufficient alignment for `usize`, because it's a weird edge case.
    if len < USIZE_SIZE
        || len < align_offset
        || USIZE_SIZE < mem::align_of::<usize>()
    {
        println!("Too small to vecterize: {:?}", &s);
        return s.iter().all(|b| b.is_ascii());
    }

    // We always read the first word unaligned, which means `align_offset` is
    // 0, we'd read the same value again for the aligned read.
    let offset_to_aligned = if align_offset == 0 {
        USIZE_SIZE
    } else {
        align_offset
    };

    let start = s.as_ptr();
    // SAFETY: We verify `len < USIZE_SIZE` above.
    let first_word = unsafe { (start as *const usize).read_unaligned() };

    if contains_nonascii(first_word) {
        return false;
    }
    // We checked this above, somewhat implicitly. Note that `offset_to_aligned`
    // is either `align_offset` or `USIZE_SIZE`, both of are explicitly checked
    // above.
    debug_assert!(offset_to_aligned <= len);

    // word_ptr is the (properly aligned) usize ptr we use to read the middle chunk of the slice.
    let mut word_ptr = unsafe { start.add(offset_to_aligned) as *const usize };

    // `byte_pos` is the byte index of `word_ptr`, used for loop end checks.
    let mut byte_pos = offset_to_aligned;

    // Paranoia check about alignment, since we're about to do a bunch of
    // unaligned loads. In practice this should be impossible barring a bug in
    // `align_offset` though.
    debug_assert_eq!((word_ptr as usize) % mem::align_of::<usize>(), 0);

    while byte_pos <= len - USIZE_SIZE {
        debug_assert!(
            // Sanity check that the read is in bounds
            (word_ptr as usize + USIZE_SIZE) <= (start.wrapping_add(len) as usize) &&
            // And that our assumptions about `byte_pos` hold.
            (word_ptr as usize) - (start as usize) == byte_pos
        );

        // 👍 Safety
        // We know `word_ptr` is properly aligned (because of
        // `align_offset`), and we know that we have enough bytes between
        // `word_ptr` and the end
        let word = unsafe { word_ptr.read() };
        if contains_nonascii(word) {
            return false;
        }

        byte_pos += USIZE_SIZE;
        // SAFETY: We know that `byte_pos <= len - USIZE_SIZE`, which means that
        // after this `add`, `word_ptr` will be at most one-past-the-end.
        word_ptr = unsafe { word_ptr.add(1) };
    }

    // If we have anything left over, it should be at-most 1 usize worth of bytes,
    // which we check with a read_unaligned.
    if byte_pos == len {
        return true;
    }

    // Sanity check to ensure there really is only one `usize` left. This should
    // be guaranteed by our loop condition.
    debug_assert!(byte_pos < len && len - byte_pos < USIZE_SIZE);

    // SAFETY: This relies on `len >= USIZE_SIZE`, which we check at the start.
    let last_word = unsafe {
        (start.add(len - USIZE_SIZE) as *const usize).read_unaligned()
    };

    !contains_nonascii(last_word)
}

///
/// 🚧 Questions
/// 1. is there value in replacing the value of the index from something that encodes the index of
///    the char (utf8) position in the file, vs a pointer, a memory address.
/// 2. next step: use an iterator to access the records and fields therein
/// 3. next step: use the first record to count the number of fields.
///

/// Core
/// Reader that drives the consumption of the data input.  It delegates the work
/// based on the memory alignment capacity.
///
/// Accordingly, there are two approaches:
/// * platform-specific vectorized computation
/// * more of a scalar approach
///
pub fn read(memmap: &Mmap) -> StructureIndex {
    // inventory of bytes
    let bytes = memmap;
    #[cfg(debug_assertions)]
    {
        println!(
            "---------------------------------------------------------------"
        );
        println!("👉 Mmap");
        println!(
            "---------------------------------------------------------------"
        );
        let rpt = ByteReport::new(&bytes);
        println!("Mmap:\n{}", &rpt);

        println!(
            "---------------------------------------------------------------"
        );
        dbg!(&bytes);
        dbg!(mem::size_of_val(&bytes));
    }

    // ----------------------------------------------------------------------
    // ⚙️  generate the splits of how to process
    // 🔑 Mmap::align_to
    //
    // prefix: &[u8]
    // shorts: &[_128]
    // suffix: &[u8] with max length of 15
    //
    let (head_u8, body_vectors, tail_u8) =
        unsafe { bytes.align_to::<__m128>() };
    // Report memory fragments
    #[cfg(debug_assertions)]
    {
        println!(
            "---------------------------------------------------------------"
        );
        println!("👉 align_to splits");
        println!(
            "---------------------------------------------------------------"
        );
        // head_u8
        println!(
            "📋 head_u8 len: {}\n{}",
            head_u8.len(),
            ByteReport::_u8_as_str(head_u8)
        );
        let input = unsafe { head_u8.get_unchecked(0 as usize..) };
        println!("{:?}", &input);

        // body_vectors
        println!(
            "📋 body_vectors len: {}\n{}",
            body_vectors.len(),
            ByteReport::_m128_as_str(body_vectors)
        );
    };

    // state
    // single allocations
    let num_vectors = body_vectors.len();
    let mut simdinput_cnt = 0;
    let mut codepoint_cnt = 0;
    let mut set_bits: u64 = 0;
    // initialize the structure index with zero as the first value
    let mut struct_acc = vec![0];
    let mut array_idx = 1; // struct_acc.len()
    let mut inside_str = 0;

    let iter_cnt = if num_vectors < INPUT_LENGTH {
        0
    } else {
        num_vectors - INPUT_LENGTH
    };

    #[cfg(debug_assertions)]
    println!("⚠️  num_vectors: {} len {}", num_vectors, iter_cnt);

    while simdinput_cnt <= iter_cnt {
        // load a 64-byte slice of the data into the registers
        let input = unsafe {
            SimdInput::new(body_vectors.get_unchecked(simdinput_cnt as usize..))
        };

        #[cfg(debug_assertions)]
        input.show();

        // transform the 64-bytes -> 64-bit structure
        input.structure(&mut set_bits, &mut inside_str);
        SimdInput::crush_set_bits(
            &mut struct_acc,
            set_bits,
            codepoint_cnt,
            &mut array_idx,
        );

        #[cfg(debug_assertions)]
        {
            println!(
                "🟢 simdinput_cnt: {} of len: {}",
                simdinput_cnt, iter_cnt
            );
            input.show();
        }

        simdinput_cnt += INPUT_LENGTH; // simdinput => raw input
        codepoint_cnt += 64; // codepoint => setbits
    }

    #[cfg(debug_assertions)]
    debug_assert_eq!(
        num_vectors - simdinput_cnt,
        num_vectors % INPUT_LENGTH,
        "The input vectors are not being processed as expected"
    );
    println!("---------------------------------------------------------------------------------");
    println!(
        "🏁 simdinput_cnt: {} of vectors: {}",
        simdinput_cnt, num_vectors
    );
    // 🔑 Maintain continuity of the memory whilst padding to SimdInput
    // load the remaining 128
    // load tail (0-16 x u8)
    let padded_input = unsafe {
        SimdInput::new_with_padding(
            body_vectors.get_unchecked(simdinput_cnt as usize..),
            tail_u8,
        )
    };

    // reset the set_bits b/c the logic relies on any unused
    // memory be set to zero.
    set_bits = 0;
    padded_input.structure(&mut set_bits, &mut inside_str);
    SimdInput::crush_set_bits(
        &mut struct_acc,
        set_bits,
        codepoint_cnt,
        &mut array_idx,
    );

    #[cfg(debug_assertions)]
    println!(
        "\n📋\ntail_u8 len: {}\n{}",
        tail_u8.len(),
        ByteReport::_u8_as_str(tail_u8)
    );

    // 🎉 The index result!
    #[cfg(debug_assertions)]
    {
        println!("🎉 index:\n{:?}", struct_acc);
        println!("len: {:?}", struct_acc.len());
    }
    StructureIndex(cast_vec(struct_acc))
}

#[cfg(test)]
mod tests {
    use crate::reader;
    use crate::stage1::StructureIndex;
    use memmap::Mmap;

    #[test]
    fn it_works_reader() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn mk_index() {
        let file = std::fs::File::open("./res/reader_test01.csv").unwrap();
        let memmap = unsafe { Mmap::map(&file).unwrap() };
        let StructureIndex(index) = reader::read(&memmap);
        let cnt = index.len();
        println!("result: {:?}", index);
        assert_eq!(4 as usize, *index[1], "The first structure pos: 4");
        assert_eq!(95 as usize, *index[cnt - 1], "The last structure pos: 95");
    }
}
