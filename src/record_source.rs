use std::fmt;

use crate::error::StructureError;
use crate::stage1::{KeyToPos, NewLine, StructureIndex};

pub struct WithRecordSource<T>(pub T);

impl<T> fmt::Display for WithRecordSource<T>
where
    T: RecordSource,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let go = || -> Result<_, StructureError> {
            let jump = *self.0.record_jump_size()?;
            Ok((
                &self.0.index().len() - 1,
                (&self.0.index().len() - 1) / jump,
                (&self.0.index().len() - 1) % jump,
            ))
        };
        match go() {
            Err(e) => write!(f, "{}", e),
            Ok((len, count, problem)) => {
                let last_record = count as u32 - 2; // 🦀  remove header
                writeln!(f, "📋 Index properties")?;
                writeln!(
                    f,
                    "len: {} records: {} problem: {}",
                    len, count, problem
                )?;
                writeln!(f, "first: {:?}", self.0.seek_record(0))?;
                writeln!(f, "last:  {:?}", self.0.seek_record(last_record))?;
                Ok(())
            }
        }
    }
}
/*
impl<T> WithRecordSource<T>
where
    T: RecordSource,
{
    pub(crate) fn new(inner: T) -> Self {
        Self(inner)
    }
}
*/

// ------------------------------------------------------------------------------
// Hack to WithRecordSource -> Tape
// ------------------------------------------------------------------------------
/*
impl<T> std::ops::Deref for WithRecordSource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> std::ops::DerefMut for WithRecordSource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
*/
// ------------------------------------------------------------------------------

pub trait RecordSource {
    /// This is not the intended use of the Tape.  It is for debugging purposes only.
    fn seek_record(
        &self,
        record_idx: u32,
    ) -> Result<Option<&str>, StructureError> {
        // The index has the memmap offset values
        // Which index value points to the start of the record?
        // record 0 = start of the memmap + header offset
        if record_idx + 1
            >= self.record_cnt().ok_or(StructureError::InvalidState)?
        {
            return Ok(None);
        };
        let field_cnt = self.field_cnt();
        let idx_start = (record_idx + 1) * (*self.record_jump_size()?) as u32;

        #[cfg(debug_assertions)]
        {
            println!("Seek record: {}", record_idx);
            println!("field count: {}", &field_cnt);
            println!("row size: {:?}", &self.record_jump_size());
            println!("idx start: {}", &idx_start);
            println!("idx end: {}", &idx_start + field_cnt);
        }

        let mem_start = self.index()[idx_start as usize];
        let mem_end = self.index()[idx_start as usize + field_cnt as usize];

        Ok(Some(unsafe {
            std::str::from_utf8_unchecked(
                &self.data_bytes()[*mem_start + 1..*mem_end],
            )
        }))
    }
    /// random-access
    fn seek_field(
        &self,
        record_idx: u32,
        field_idx: u32,
    ) -> Result<Option<&str>, StructureError> {
        // The index has the memmap offset values
        // Which index value points to the start of the record?
        // record 0 = start of the memmap + header offset
        if record_idx + 1
            >= self.record_cnt().ok_or(StructureError::InvalidState)?
        {
            return Ok(None);
        };
        if field_idx >= self.field_cnt() {
            return Ok(None);
        };
        let field_cnt = self.field_cnt();
        println!("Seek record: {}", record_idx);
        println!("field count: {}", &field_cnt);
        let row_size = match self.new_line_tag() {
            NewLine::CRLF => field_cnt + 1,
            _ => field_cnt,
        };
        println!("row size: {}", &row_size);
        //
        let idx_start = (record_idx + 1) * row_size + field_idx;
        println!("idx start: {}", &idx_start);
        println!("idx end: {}", &idx_start + 1);
        let mem_start = self.index()[idx_start as usize];
        let mem_end = self.index()[idx_start as usize + 1];

        Ok(Some(unsafe {
            std::str::from_utf8_unchecked(
                &self.data_bytes()[(*mem_start) + 1..*mem_end],
            )
        }))
    }
    fn record_cnt(&self) -> Option<u32>;
    fn index(&self) -> &StructureIndex;
    fn record_jump_size(&self) -> Result<KeyToPos, StructureError>;
    fn field_cnt(&self) -> u32;
    fn new_line_tag(&self) -> &NewLine;
    fn data_bytes(&self) -> &[u8];
}
/*
impl fmt::Debug for dyn RecordSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max_idx = &self.index().len() - 1;
        f.debug_struct("Tape")
            .field("Field_count", &self.field_cnt())
            .field("Record_size", &self.record_jump_size())
            .field("Records", &self.record_cnt())
            .field("Index_len", &self.index().len())
            .field("Index_max_value", &self.index()[max_idx])
            .finish()
    }
}
*/
