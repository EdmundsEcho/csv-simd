use core::arch::x86_64::*;

#[derive(Debug)]
pub struct ByteReport<'data> {
    len: u32,
    bytes: &'data [u8],
}
/// bytes -> Report
impl<'a> ByteReport<'a> {
    /// bytes -> Report
    pub fn new(input: &'a [u8]) -> Self {
        ByteReport {
            len: input.len() as u32,
            bytes: input,
        }
    }
    /// a function; bytes -> &str for display input
    pub fn _u8_as_str(input: &'a [u8]) -> &'a str {
        std::str::from_utf8(input).expect("Fond invalid UTF-8")
    }
    /// a function; bytes -> &str for display input
    pub fn _m128_as_str(input: &'a [__m128]) -> &'a str {
        let tmp: &[u8] = unsafe { std::mem::transmute(input) };
        std::str::from_utf8(tmp).expect("Fond invalid UTF-8")
    }
}

impl std::fmt::Display for ByteReport<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.len {
            0 => writeln!(f, "empty")?,
            x if x <= 1000 => {
                let max = self.len - 1;
                let first_1k =
                    std::str::from_utf8(&self.bytes[0..max as usize]).expect("Invalid UTF-8");
                writeln!(f, "num char: {}", self.len)?;
                writeln!(f, "{}", first_1k)?;
            }
            _ => {
                let first_1k = std::str::from_utf8(&self.bytes[0..1000]).expect("Invalid UTF-8");
                let tail = std::str::from_utf8(
                    &self.bytes[self.len as usize - 101..self.len as usize - 1],
                )
                .expect("Invalid UTF-8");

                writeln!(f, "num char: {}", self.len)?;
                writeln!(f, "{}...\n...{}", first_1k, tail)?;
            }
        }

        Ok(())
    }
}
