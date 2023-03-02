/// ⬜ Formally apply the builder patter
/// https://rust-unofficial.github.io/patterns/patterns/builder.html
///
// use bytemuck::cast;
use memmap::Mmap;
use std::fmt;

use crate::error::StructureError;
use crate::record_source::{RecordSource, WithRecordSource};
use crate::stage1::{KeyToPos, NewLine, StructureIndex};

/// Atomic representation of how to utilize the tape in a parallel-processing context.
pub struct Chunk<'index> {
    pub id: u8,
    pub start: KeyToPos,
    pub end: KeyToPos,
    pub record_cnt: u32,
    pub index: &'index StructureIndex,
}
impl<'index> Chunk<'index> {
    pub fn show(&self) {
        println!("chunk {} {}", self.start, self.end);
    }
}
impl<'index> fmt::Debug for Chunk<'index> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let start_idx = self.start;
        let end_idx = self.end;

        f.debug_struct("Chunk")
            .field("id", &self.id)
            .field("records", &self.record_cnt)
            .field("record first", &start_idx)
            .field("last", &end_idx)
            .field("index first", &(self.index[*self.start]))
            .field("last", &(self.index[*self.end]))
            .finish()
    }
}
pub type Chunks<'index> = Vec<Chunk<'index>>;

/// A slice representation of the data source.  The stride of each index is u8 representing UTF8.
pub struct DataBytes(Mmap);
// ------------------------------------------------------------------------------
// Data trait implementations
// ------------------------------------------------------------------------------
impl fmt::Display for DataBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.0.len();
        write!(
            f,
            "DataBytes len: {} first: {} last: {}",
            len,
            self.0[0],
            self.0[len - 1]
        )
    }
}
impl std::ops::Deref for DataBytes {
    type Target = Mmap;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for DataBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
// ------------------------------------------------------------------------------

/// External-facing version of TapeCore
pub struct Tape {
    pub header: Header,
    pub record_cnt: u32,
    pub record_jump_size: KeyToPos,
    bytes: DataBytes,
    index: StructureIndex,
}

impl Tape {
    pub fn from_core(core: TapeCore) -> Result<Tape, StructureError> {
        let mut core = core;
        core.init()?;

        Ok(Tape {
            header: core.header,
            bytes: DataBytes(core.memmap),
            record_cnt: core.record_cnt.unwrap(), // safe with init
            record_jump_size: core.record_jump_size.unwrap(), // safe with init
            index: core.index,
        })
    }
    pub fn chunks<'index>(
        &'index self,
        num: u8,
    ) -> Result<Chunks<'index>, StructureError> {
        let mut chunks = boundaries(self.record_cnt, num)
            .ok_or(StructureError::InvalidState)?
            .iter()
            .enumerate()
            .map(|(id, boundary)| Chunk {
                id: id as u8,
                start: KeyToPos(
                    boundary.start * *self.record_jump_size as usize,
                ),
                end: KeyToPos(
                    (boundary.start + boundary.len)
                        * *self.record_jump_size as usize,
                ),
                record_cnt: boundary.len as u32,
                index: &self.index,
            })
            .collect::<Vec<Chunk>>();

        chunks[0] = Chunk {
            id: chunks[0].id,
            start: self.record_jump_size,
            end: chunks[0].end,
            record_cnt: chunks[0].record_cnt - 1,
            index: chunks[0].index,
        };

        /*
        let last = (num - 1) as usize;
        assert_eq!(
            self.data.len(),
            chunks[last].end,
            "The last index is not aligned with the data"
        );
        chunks[last] = Chunk {
            start: chunks[last].start,
            end: self.data.len(),
            index: chunks[last].index,
        };
        */

        Ok(chunks)
    }
    pub fn index(&self) -> &StructureIndex {
        &self.index
    }
    pub fn bytes(&self) -> &DataBytes {
        &self.bytes
    }
    pub fn as_records(&self) -> WithRecordSource<&Tape> {
        todo!()
    }
    pub fn header(&self) -> &Vec<String> {
        &self.header.header
    }
}

impl RecordSource for &Tape {
    fn record_cnt(&self) -> Option<u32> {
        Some(self.record_cnt)
    }
    fn index(&self) -> &StructureIndex {
        &self.index
    }
    fn record_jump_size(&self) -> Result<KeyToPos, StructureError> {
        Ok(self.record_jump_size)
    }
    fn field_cnt(&self) -> u32 {
        self.header.field_cnt
    }
    fn new_line_tag(&self) -> &NewLine {
        &self.header.new_line
    }
    fn data_bytes(&self) -> &[u8] {
        (*self.bytes).as_ref()
    }
}

use std::fmt::Display;
impl fmt::Debug for Tape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        WithRecordSource(self).fmt(f)
    }
}
/// A intermediate, internal representation of the primary interface.
/// WIP: Temporary access in order to peek at specific records and fields.
#[derive(Debug)]
pub struct TapeCore {
    header: Header,
    index: StructureIndex,
    memmap: Mmap,
    first_record_idx: Option<usize>,
    record_cnt: Option<u32>,
    record_jump_size: Option<KeyToPos>,
}
impl RecordSource for TapeCore {
    fn record_cnt(&self) -> Option<u32> {
        self.record_cnt
    }
    fn index(&self) -> &StructureIndex {
        &self.index
    }
    fn record_jump_size(&self) -> Result<KeyToPos, StructureError> {
        self.record_jump_size.ok_or(StructureError::InvalidState)
    }
    fn field_cnt(&self) -> u32 {
        self.header.field_cnt
    }
    fn new_line_tag(&self) -> &NewLine {
        &self.header.new_line
    }
    fn data_bytes(&self) -> &[u8] {
        &self.memmap
    }
}

/// Vec of field names
/// ⬜ The delimiter value is not referencing a single value and is fixed ','
#[derive(Debug)]
pub struct Header {
    pub header: Vec<String>,
    new_line: NewLine,
    pub field_cnt: u32,
    delimiter: u8, // utf8
    pub record_offset: u32,
}

impl Header {
    pub fn new(memmap: &Mmap) -> Header {
        // end of the header
        let header_end_idx = memmap
            .iter()
            .take_while(|&code_point| *code_point != 0xd && *code_point != 0xa)
            .collect::<Vec<_>>()
            .len();

        // Set the NewLine value
        let mut new_line = NewLine::LF;
        if memmap[header_end_idx + 1] == 0xa {
            new_line = NewLine::CRLF;
        };

        // skip the bit-order-marker (if exists)
        let header_start_idx = memmap
            .iter()
            .take_while(|&code_point| {
                *code_point == 0xef
                    || *code_point == 0xbb
                    || *code_point == 0xbf
            })
            .collect::<Vec<_>>()
            .len();

        let header = unsafe {
            std::str::from_utf8_unchecked(
                &memmap[header_start_idx..header_end_idx],
            )
        };

        // ⚠️  Memory allocation
        // 🦀 depends on the split delimiter, not yet set
        let header = header
            .split(",")
            .map(|name| name.trim().to_string())
            .collect::<Vec<String>>();

        let field_cnt = header.len() as u32;

        Header {
            header,
            new_line,
            field_cnt,
            delimiter: 0x2C,
            record_offset: header_end_idx as u32,
        }
    }
    pub fn field_cnt(&self) -> u32 {
        self.field_cnt
    }
}

/// Generic boundary in the Tape.index
#[derive(Debug, PartialEq)]
pub struct Boundary {
    pub start: usize,
    pub len: usize,
}

// 🚧 How read the tape?
// "this", "that","done",eol
// 0,23,34,45,66,67
//
// Given the host of the data: Memmap
//
// Option 1: iterators = # of fields + one to iterate on the counters
// Vec<Counter> that reads left-to-right, whilst iterating through the counters
//
// Option 2: iterators = # of fields
// Counter that jumps through the Tape separate from the other counter.
//
// Shared features:
// Each counter jumps forward by number of fields

impl TapeCore {
    /// ⬜ Configure whether the Header is included in the memmap and index
    pub fn create(memmap: Mmap, index: StructureIndex, header: Header) -> Self {
        TapeCore {
            header,
            index,
            memmap,
            first_record_idx: None,
            record_cnt: None,
            record_jump_size: None,
        }
    }
    /// Compute the record_size and record_cnt.  WIP: compute the location of the first record when
    /// the presence of a Header is optional.
    pub(crate) fn init(&mut self) -> Result<(), StructureError> {
        // tasks conpute record_size and record_count
        // None -> Some jump_size
        self.record_jump_size = match self.header.new_line {
            NewLine::CRLF => Some(KeyToPos(self.header.field_cnt as usize + 1)),
            _ => Some(KeyToPos(self.header.field_cnt as usize)),
        };

        self.record_cnt = Some(
            ((self.index.len() - 1) / *self.record_jump_size.unwrap()) as u32,
        );

        let problem = (self.index.len() - 1) % *self.record_jump_size.unwrap();

        #[cfg(debug_assertions)]
        {
            println!("-------------------------------------------------");
            println!("🚧 CoreTape properties");
            println!("NewLine {:?}", self.header.new_line);
            println!("field cnt {}", self.header.field_cnt);
            println!("jump_size {}", self.record_jump_size.unwrap());
            println!("index.len() {}", self.index.len());
            println!("constant record size? {} {}", problem == 0, problem);
            println!("record count: {}", self.record_cnt.unwrap());
            println!("-------------------------------------------------");
        }

        if problem != 0 {
            return Err(StructureError::InvalidCsvFormat);
        };

        Ok(())
    }
    /// show the header
    pub fn header(&self) -> &[String] {
        self.header.header.as_slice()
    }
}

/// Divide a task into the desired number of jobs. Each job is specified as a boundary.  The units
/// used to describe the 'task_size' specify the units used to describe a unit in the boundary.
/// e.g., task_size = 1000 lines, a boundary specifies the number of lines = end - start.
///
/// The boundary specifies a zero-index range, with an inclusive start and exclusive end value.
///
/// # Examples
///
/// ```
/// # use csv_simd::tape::boundaries;
/// # use csv_simd::tape::Boundary;
///
/// let result = boundaries(8, 3).unwrap();
/// assert_eq!(result[0], Boundary { start:0, len: 3 });
/// assert_eq!(result[1], Boundary { start:3, len: 3 });
/// assert_eq!(result[2], Boundary { start:6, len: 2 });
/// assert_eq!(8, result.iter().map(|boundary| boundary.len as i32).sum());
///
/// let result = boundaries(1000, 12).unwrap();
/// assert_eq!(result[0], Boundary { start:0, len: 84 });
/// assert_eq!(result[1], Boundary { start:84, len: 84 });
/// assert_eq!(result[11], Boundary { start:917, len: 83 });
/// assert_eq!(1000, result.iter().map(|boundary| boundary.len as i32).sum());
///
/// let result = boundaries(8, 12).unwrap();
/// assert_eq!(result[0], Boundary { start:0, len: 8 });
/// assert_eq!(8, result.iter().map(|boundary| boundary.len as i32).sum());
///
/// let result = boundaries(0, 3);
/// assert!(result.is_none());
/// ```
pub fn boundaries(task_size: u32, job_count: u8) -> Option<Vec<Boundary>> {
    //
    if task_size == 0 || job_count == 0 {
        return None;
    }
    if task_size < job_count as u32 {
        return Some(vec![Boundary {
            start: 0,
            len: task_size as usize,
        }]);
    }

    // sub-routine
    let boundaries = {
        //
        #[cfg(debug_assertions)]
        {
            println!("Creating a slices of work");
            println!("Job count: {} Task size: {}", job_count, task_size);
        }
        let job_size = dbg!(task_size / job_count as u32);
        let remainder = task_size % job_count as u32;

        let mut boundaries = Vec::with_capacity(job_count as usize);
        let mut acc_end = 0;
        let mut share_remainder = 1;

        for i in 0..job_count {
            if share_remainder == 1 && i >= remainder as u8 {
                share_remainder = 0;
            }
            boundaries.push(Boundary {
                start: acc_end as usize,
                len: (job_size + share_remainder) as usize,
            });
            acc_end += job_size + share_remainder;
        }
        boundaries
    };
    #[cfg(debug_assertions)]
    println!("Slices of work:\n{:?}", boundaries);

    Some(boundaries)
}
