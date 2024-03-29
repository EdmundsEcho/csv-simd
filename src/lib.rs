pub use memmap::Mmap;
use std::fs::File;
use std::time::Instant;

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
// ------------------------------------------------------------------------------------------------
/*
use jemallocator;
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;
*/
// ------------------------------------------------------------------------------------------------
/// avx-related; likely use SSE2
// #[cfg(target_feature = "avx")]
mod avx;

/// Generic support for the Stage1 processing of a CSV file
pub(crate) mod stage1;
pub use crate::stage1::StructureIndex;

pub mod record_source;
pub use crate::record_source::{RecordSource, WithRecordSource};

/// haystack
pub mod reader;

/// Start processing raw data
mod structure;

/// value/tape.rs
pub mod tape;
pub use crate::tape::{Header, Tape, TapeCore};

/// error
mod error;
pub use crate::error::StructureError;

/// temporary level-setting that replicates the Lemir json work
mod json_test;

/// helper functions
mod helper;
pub use helper::ByteReport;

// A tag that sometimes prefixes data sources to be ignored by the csv app.
// const UTF8_BOM: &'static [u8] = b"\xef\xbb\xbf";

#[allow(dead_code)]
#[cfg(debug_assertions)]
static PATH: &str = "./res/sample_rx.csv";
// static PATH: &str = "./res/sample.csv";

#[cfg(not(debug_assertions))]
static PATH: &str = "/Users/edmund/Desktop/data/warfarin_NRx.csv";

// 🚧  The lib factory
//
/// Create a Tape from a filename
pub fn create(filename: &str) -> Result<Tape, StructureError> {
    let now = Instant::now();

    let file = File::open(filename)?;
    let memmap = unsafe { Mmap::map(&file)? };
    let header = tape::Header::new(&memmap);
    let index = reader::read(&memmap);
    let tape = TapeCore::create(memmap, index, header);
    let tape = Tape::from_core(tape)?;

    println!("Elapsed: {} seconds", now.elapsed().as_secs_f64());

    Ok(tape)
}

/*
pub fn run() -> io::Result<()> {
    // level-set
    // json_test::run()?;

    #[cfg(not(debug_assertions))]
    {
        dbg!(tape.seek_record(0));
        dbg!(tape.seek_record(100));
        dbg!(tape.seek_record(2889));
        dbg!(tape.seek_record(3243));
        dbg!(tape.seek_record(3244));
        dbg!(tape.seek_record(3245));
        dbg!(tape.seek_field(3245, 7));
        dbg!(tape.seek_field(3245, 8));
    }
    #[cfg(debug_assertions)]
    {
        dbg!(tape.seek_record(0));
        dbg!(tape.seek_record(1));
        dbg!(tape.seek_record(2));
        dbg!(tape.seek_record(3));
        dbg!(tape.seek_record(4));
        dbg!(tape.seek_record(5));
        dbg!(tape.seek_record(6));
        dbg!(tape.seek_record(7));
        dbg!(tape.seek_record(10));
        dbg!(tape.seek_field(10, 1));
        dbg!(tape.seek_field(6, 0));
        dbg!(tape.seek_field(6, 1));
        dbg!(tape.seek_field(6, 2));
        dbg!(tape.seek_field(6, 3));
        dbg!(tape.seek_field(6, 7));
        dbg!(tape.seek_field(6, 8));
    }
}
*/

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn binary_manipulations() {
        // 📚 ...isolate the lowest set bit
        // create an index
        // low nib for 'n' = e
        // lowest set bit
        // 0100100 start
        // 1011011 compliment
        // 1011100 plus 1
        //
        // 1011100 plus 1
        // 0100100 original AND
        // 0000100 goal = lowest set bit
        //
        // 📚 ...remove the lowest set bit
        // remove the lowest set bit
        // 0b01011100 start
        // 0b01011011 ...minus 1
        // 0b01011000 AND
        let test: u8 = 0b01011100;
        let test_minus_1: u8 = test - 1;
        assert_eq!(test_minus_1, 0b01011011);
        let result = test & test_minus_1;
        assert_eq!(
            result, 0b01011000,
            "The result with the lowest set bit removed"
        );
        let test: u8 = 0b01011100;
        assert_eq!(
            test.saturating_sub(1) & test,
            0b01011000,
            "The result with the lowest set bit removed"
        );
    }
}
