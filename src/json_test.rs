use std::fs::File;
use std::io;

use memmap::Mmap;

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use core::mem;

static PATH: &str = "./res/json_test.txt";

pub fn run() -> io::Result<()> {
    let file = File::open(PATH)?;
    let map = unsafe { Mmap::map(&file)? };
    println!("Map with len {} created.", map.len());

    // -----------------------------
    use std::str;
    let sample = &map[0..5];
    println!("The sample");
    println!("{:?}", sample);

    let buff = str::from_utf8(sample).expect("Found invalid UTF-8");
    println!("{:?}", buff);

    // -----------------------------
    // is_ascii
    use crate::reader;
    println!("Is valid ascii: {:?}", reader::is_ascii(&map[0..]));
    println!("Is valid ascii: {:?}", reader::is_ascii(&map[0..7]));

    println!("----------------------------------------------------------------");
    println!("Identify json structure");
    unsafe {
        macro_rules! low_nibble_struct {
            () => {
                _mm_setr_epi8(16, 0, 0, 0, 0, 0, 0, 0, 0, 8, 10, 4, 1, 12, 0, 0)
            };
        }
        macro_rules! high_nibble_struct {
            () => {
                _mm_setr_epi8(8, 0, 17, 2, 0, 4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0)
            };
        }

        println!("---------------------------------------------------");
        println!("the three constants");
        println!("---------------------------------------------------");
        // the three constants
        // json structure
        let lo_nibble_mask: __m128i = low_nibble_struct!();
        let hi_nibble_mask: __m128i = high_nibble_struct!();
        // Broadcast 8-bit integer a to all elements of dst.
        let low_mask: __m128i = _mm_set1_epi8(0xf);

        let temp = mem::transmute::<_, [u8; 16]>(lo_nibble_mask);
        println!("low_nibble_mask {:?}", &temp);
        let temp = mem::transmute::<_, [u8; 16]>(hi_nibble_mask);
        println!("high_nibble_mask {:?}", &temp);
        let temp = mem::transmute::<_, [u8; 16]>(low_mask);
        println!("low_mask {:?}", &temp);

        println!("---------------------------------------------------");
        println!("test bit shifting");
        let test: [u8; 16] = [
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        ];
        let ptr: &[u8] = &test;
        let test_load = _mm_loadu_si128(ptr.as_ptr() as *const __m128i);
        let temp = mem::transmute::<_, [u8; 16]>(test_load);
        println!("test_load   {:?}", &temp);

        // bit-shift is a two-step process
        // step 1: only matters that we >> 4
        let test_shift = _mm_srli_epi64(mem::transmute(test_load), 4);
        let temp = mem::transmute::<_, [u8; 16]>(test_shift);
        println!("test_shift  {:?}", &temp);

        // step 2: apply low-mask to emulate replacing with zero
        let test_shift: __m128i = _mm_and_si128(test_shift, low_mask);
        let temp = mem::transmute::<_, [u8; 16]>(test_shift);
        println!("test_shift  {:?}", &temp);
        println!("---------------------------------------------------");

        // input 8 x 16-bit
        let chunk = &map[0..16];
        let buff = str::from_utf8(chunk).expect("Found invalid UTF-8");
        println!("buff {:?}", &buff);
        println!("chunk {:?}", &chunk);
        let ptr: &[u8] = chunk;
        let chunk = _mm_loadu_si128(ptr.as_ptr() as *const __m128i);

        // low nib = bitwise AND with low_mask
        // Compute the bitwise AND of 128 bits (representing integer data) in a and b,
        // and store the result in dst.
        let nib_lo: __m128i = _mm_and_si128(chunk, low_mask);
        let temp = mem::transmute::<_, [u8; 16]>(nib_lo);
        println!("nib_lo  {:?}", &temp);

        // high nib = shift 4, apply low_mask
        let nib_hi = _mm_srli_epi64(mem::transmute(chunk), 4);
        let nib_hi = _mm_and_si128(nib_hi, low_mask);
        let temp = mem::transmute::<_, [u8; 16]>(nib_hi);
        println!("nib_hi  {:?}", &temp);

        // lookup
        // The last 4 bits of each byte of b are used as addresses into the 16 bytes of a.
        // ... low nibble -> byte value (vec: lookup 16 times)
        let shuf_lo = _mm_shuffle_epi8(lo_nibble_mask, nib_lo);
        // ... high nibble -> byte value
        let shuf_hi = _mm_shuffle_epi8(hi_nibble_mask, nib_hi);

        let temp = mem::transmute::<_, [u8; 16]>(shuf_lo);
        println!("shuf_lo {:?}", &temp);
        let temp = mem::transmute::<_, [u8; 16]>(shuf_hi);
        println!("shuf_hi {:?}", &temp);

        // combine lo/hi
        let result = _mm_and_si128(shuf_lo, shuf_hi);
        let temp = mem::transmute::<_, [u8; 16]>(result);
        println!("result  {:?}", &temp);

        // assertions
        // given...
        let mask: u8 = 0xf;
        let win: u8 = 123;
        let low = (mask & win) as usize;
        assert_eq!(low, 0xb as usize, "Isolated test");
        assert_eq!(low, 11 as usize, "Isolated test");

        // low nib for { = b
        assert_eq!(
            mem::transmute::<_, [u8; 16]>(nib_lo)[0],
            0xb,
            "{{: Testing the low nib value"
        );
        // high nib for { = 7
        assert_eq!(
            mem::transmute::<_, [u8; 16]>(nib_hi)[0],
            0x7,
            "{{: Testing the high nib value"
        );
        // low nib for a = 1
        assert_eq!(
            mem::transmute::<_, [u8; 16]>(nib_lo)[2],
            1,
            "a: Testing the low nib value"
        );
        // high nib for a = 6
        assert_eq!(
            mem::transmute::<_, [u8; 16]>(nib_hi)[2],
            0x6,
            "a: Testing the high nib value"
        );
    }

    Ok(())
}
