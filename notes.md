## Decision on how to build index

  1. access using a slice

     👉 &[0..5]  => field_0: start, end
     👉 &[5..11] => field_1: start, end
     👉 etc...
     👉 zero = record start position

  2. For data analysis:

     👉 FIX: fixed number of fields
     👉 FIX: no escape, no comma inside quotes (For Now)
     👉 data structure for storage:
        * 1-D array
          * index = field count
            * record number = round down [field count] / [number of fields]
            * field number  = [field count] mod [number of fields]
          * value = io::offset



## Good references/resources
* [compiler and platform flags](https://rust-lang.github.io/packed_simd/perf-guide/)
* [text analysis in Rust](https://nitschinger.at/Text-Analysis-in-Rust-Tokenization/)
* echo cpu feature support: sysctl -a | grep machdep.cpu
* [on pointers to memory feature subslice_offset](https://github.com/fusion-engineering-forks/rfcs/blob/subslice-offset/text/0000-subslice-offset.md)

## Read IO
[burnsushi getting grilled](https://users.rust-lang.org/t/stream-api-and-types/18849)
Similarly, zero-copy I/O doesn’t have to be complicated, but the only way to do that is for read() to provide the buffer to the consumer instead of the consumer passing in a buffer to be filled. These are fundamentally opposite concepts, and the latter can never support the zero-copy scenario in the general-case.

"All streams can be thought of as copying into a client-provided byte buffer" is just wrong.

On how to read bytes from the syscall data: Expose slice of internal buffer has lifetime issue as this buffer should be reused. Vec implies heap allocation for every read() which is not acceptable.

Rust consumes buffers automatically.  Not ideas according to this person.  e.g., processing binary data in 1MB buffer.  Halfway through, see that it's now XML. Now what? Stuff the unconsumed data back into the previous stream level?

### Not zero copy; likely 2-3
The shim is doing buffered reading. Specifically, if the shim is wrapped around a fs::File, then:

* UTF-16 encoded bytes are copied to an internal buffer directly from a read syscall (kernel to user).
* Transcoding is performed from the bytes in the internal buffer to the caller’s buffer directly.

A perusal of the code makes it look like an additional copy is happening, but in practice, this copy is just rolling a small number of bytes from the end of the buffer to the beginning of the buffer that either couldn’t fit in the caller’s buffer or represent an incomplete UTF-16 sequence.

*...memory maps are fast*
The Read trait is just an OS independent interface that loosely describes how to read data. For example, when reading from a File, the buffer provided to the read method is going to be written to directly by the OS. That’s as little possible copying as you can do. To do better, you need to go into kernel land or use memory maps.

*... separate get a buffer from consume input*
 System.IO.Pipelines and separated the “get a buffer” and “consume input items” concepts then the BOM detection code would be hilariously trivial

*... composing Readers starting with lowest-level ByteReader*

    How would you implement this for a Read over f32?

With composed readers. ByteReader (lowest-level provided by std) -> BufferedReader (provided by std) -> FloatReader (provided by a crate/make it). Same as anywhere else.

*... the Rust Read trait cannot implement zero-copy promise*

Meanwhile, the Read trait cannot implement the more elegant zero-copy trait, because:

* It cannot read without consuming bytes.
* It cannot read non-copy types even if generalised to a template trait with a default u8 parameter.
* It breaks the performance contract of zero copy.


*... aha, but any kind of peek implies a buffer*
 Non destructive peeking means the source has a buffer (either naturally or manufactured), whereas Read is just a stream. If you want to add a buffer on top and allow peeking, you can do that yourself (or use BufReader or BufRead trait in the API requirements). BufRead has a fill_buf/consume duo that can be used to do buffering and peeking, and then advancement.

 *... the use of pipeline that supplies it's own buffer - less control*
 that any implementation of the Pipeline API will necessarily require a large internal buffer that the client can’t control.

*... memmap can be seen as a window to an infinite stream*
The “mapped window size”, and the “file size” are distinct concepts, only the former is limited to isize (not usize!). For instance, the mapped window size is limited by the OS, but that of the file/stream is not.  e.g., to offset into the file, you need to use u64 even on a 32-bit machine.  u64 make the addresses unique.  The memmap-rs crate uses usize; no bueno.  This all assumes the end-user is not setting the address of incoming memory.

*... what is &[u8] rely on?*
* the pointer to the memory start: memory-dependent, not machine dependent
* the length of the slice, limited by the machine (usize max index)
* 32-bit processes are limited to 2GB

Note: trait BufRead owns the buffer whereas Read requires that one be passed to it.

### what the API should promise
What the “API user” actually wants from any I/O is typically: “Give me as much data is efficiently available right now, and I’ll see how much I can consume, most likely all of it. Don’t stop reading just because I’m processing data.”

Read forces a guess: Too small, hammer the kernel, too big the inherent copy will blow through the "L1/L2" cache thrashing. Brutal for holding onto bytes not consumed by various layers as they reach the end of their roles.

Meanwhile, the BufRead/Read2 style of API design allows the system with the knowledge – the platform I/O library – to make the judgement call of the best buffer size. The user can provide a minimum and allow the platform to provide that plus a best-effort extra on top. The best effort can dynamically grow to be the entire file if mmap is available. Or… most of the file if mmap is available and the platform is 32-bit. The API user can then wrap this in something consuming the input byte-by-byte such a decompressor and not have to worry about the number of kernel calls. Similarly, the default non-tokio version can still use async I/O behind the scenes without the consumer being forced to use an async API themselves. It all just… works by default, as long as it is the default.

### buff size 64KB is a good starting point; 512 bytes is wayyyy too small and...
~8KB is pushing it for L1 cache, 32KB for L2 cache, and a few MB for L3 cache.
Naive copy code has not way to know how to alter the cache to help.  If too large, start dropping bytes in the stream - retransmit.

```rust
let buf = src.read(0); // we *get* a buffer instead of passing one in...
dst.write(buf); // ideally, this ought to be a 'move' so we lose control of the buffer.
src.consume(buf.len()); // just copy as much as we can, fast as we can...
let buf = src.read(0); // The magic: nobody said this is the SAME buffer!!!
dst.write(buf);  // Now we're just passing buffers like a bucket brigade...
src.consume(buf.len());
```

### using memmap is somewhat better than traditional IO but
Realistically, what tends to happen is people skip over the fiddly “window sliding” code they should be writing, incorrectly assume that mmap == &[u8], then their code can be simpler, much faster, and wrong.

[End of the stream](https://users.rust-lang.org/t/stream-api-and-types/18849)

## Using ref to ref (pointer to pointers)

source: [community post](https://users.rust-lang.org/t/signaling-partial-read-write-of-a-caller-supplied-buffer/3633/2)

```rust
fn foo(src: &mut &[u8], dst: &mut &mut [u8]) { ... }

fn bar(src: &[u8], dst: &mut [u8]) {
    let remaining_dst_len;
    {  // Limit the scope of remaining_* borrows
        let remaining_src = &src[..];
        let remaining_dst = &mut dst[..];
        while something {
            foo(&mut src, &mut dst)
        }
        remaining_dst_len = remaining_dst.len();
    }
    let written = &mut dst[..dst.len() - remaining_dst_len];
    ...
}
```

## Initial notes on method

```rust
// record where the pointer will be after processing
// let next_base = b + cnt;
//
// start with a buffer of a fixed size: 8 x 32u
// ...more often, we will not fill it.
// Mark to where we will fill it using count_ones()
// Subsequently, only advance the index for the next iteration
// accordingly.
//

// *b++ does two things
// 1. set the value *b
// 2. advance the pointer by one unit of memory
//
// 📚 Number and size of registers
// SSE   8 128-bits
// SSE2 16 128-bits
// AVX  16 256-bits

// 📚 Loop unrolling
// Memory alignment
// 👉 n/k (round down) iterations
// 👉 + n mod k
//
// 📚 Aligning data
// Why? Normally pointers work so that memory address/size -> integer (divisible).
// Vector: Jumping by chunks of 128-bits, the location of the pointer must be divisible
// by 128-bits.
//
//  ------------- - - - - - -
//                ^ ^ ^ ^ ^ ^ one of these is divisible
//  e.g., 6-byte pointer: 0x800_001
//
//  How in general find closest divisor?
//  case: 3, and divisible by 10.
//  3 + 10 = 13
//  round down to 10.
//  ... where, shift right then left by one.
//  ... or, in binary how round to 4?
//           0111
//           1011 add 4
//           1100 AND with mask "one" less than 4
//           1000 result
//
// 📖 ~0x0f -> 0b11110000
//    ... a useful mask for rounding down to 16-byte *boundary*
//    1. cast pointer to uintptr_t to enable bitwise operations to the pointer itself.
//    2. multiply (anding) the address with 15 (0xf) to mask the higher bits
//    3. the last 4 bits must be zero if is 16-byte boundary.
//
//
// This effectively means that the address of the memory your data resides in needs to be
// divisible by the number of bytes required by the instruction.
//
// The alignment is 16 bytes (128 bits), which means the memory address of your data needs
// to be a multiple of 16. E.g. 0x00010 would be 16 byte aligned, while 0x00011 would not be.
//
// How many iterations:
// ===================
// main: x = 0; x < 1003/8*8; x += 8
// tail: x = 1008/8*8; x < 1003; x++
//
// main: i=1003; i>0 && i> 1003 mod 8; i=i-8
// tail: i=1003 mod 8; i>0; i--
//
// Sum of [i32]
// for i=0; n/4*4; i=i+4 { add 4 ints with 128-bits from &a[i] to temp; }
// tail; copy out 4 integers of temp and add them together to sum
// for(i=n/4*4; i<n; i++) { sum += a[i]; }
```

## Sample of how to build an index

     record = csv::ByteRecord::new();
     while rdr.read_byte_record(&mut record)?
            let pos = record.position().expect("position on row");
            wtr.write_u64::<BigEndian>(pos.byte())?;

```rust
pub fn create<R: io::Read>(
        rdr: &mut csv::Reader<R>,
        mut wtr: W,
    ) -> csv::Result<()> {
        // If the reader is configured to read a header, then read that
        // first. (The CSV reader otherwise won't yield the header record
        // when calling `read_byte_record`.)
        let mut len = 0;
        if rdr.has_headers() {
            let header = rdr.byte_headers()?;
            if !header.is_empty() {
                let pos = header.position().expect("position on header row");
                wtr.write_u64::<BigEndian>(pos.byte())?;
                len += 1;
            }
        }

        //
        // ✨
        // 👉 Single instance of record
        // 👉 Reader feeds instance
        //    * attributes of the status includes position
        //      record.position()
        //      … a custom entry: Position
        //      🛈  Is this something the reader provides?
        //
        // 🔑 Reader is an iterator when it is feeding the record.
        //
        let mut record = csv::ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            let pos = record.position().expect("position on row");
            wtr.write_u64::<BigEndian>(pos.byte())?;
            len += 1;
        }
        wtr.write_u64::<BigEndian>(len)?;
        Ok(())
    }

```

The process of setting a record value when iterating the reader.

```rust

    // 📖 in the implementation of csv::Reader

    // uses a side-effect to update the record

    pub fn read_byte_record(
        &mut self,
        record: &mut ByteRecord,
    ) -> Result<bool> {
        if !self.state.seeked && !self.state.has_headers && !self.state.first {
            if let Some(ref headers) = self.state.headers {
                self.state.first = true;

                // 👉 record is set here
                record.clone_from(&headers.byte_record);

                if self.state.trim.should_trim_fields() {
                    record.trim();
                }
                return Ok(!record.is_empty());
            }
        }

        // 👉 record can be set here too
        let ok = self.read_byte_record_impl(record)?;
        self.state.first = true;

        if !self.state.seeked && self.state.headers.is_none() {
            self.set_headers_impl(Err(record.clone()));
            if self.state.has_headers {

                // 👉 record can be set here too
                let result = self.read_byte_record_impl(record);

                if self.state.trim.should_trim_fields() {
                    record.trim();
                }

                return result;
            }
        } else if self.state.trim.should_trim_fields() {

            // 👉 record can be set here too
            record.trim();
        }
        Ok(ok)
    }

    // 🔑 record position is set by the reader
    //    within read_byte_record_impl the record is updated as a side effect
    record.set_position(Some(self.state.cur_pos.clone()));

    // csv::Reader as self.state.cur_pos

    state: ReaderState {
           headers: None,
           has_headers: builder.has_headers,
           flexible: builder.flexible,
           trim: builder.trim,
           first_field_count: None,
           cur_pos: Position::new(),  // 👉
           first: false,
           seeked: false,
           eof: ReaderEofState::NotEof,
    },

    // 📖 pub fn seek(&mut self, pos: Position) -> Result<()> {..}
    // pos in relation to the Rust std::io::SeekFrom.
    // 👎 Note how it seeks from the start of the file `Start`

    self.rdr.seek(io::SeekFrom::Start(pos.byte()))?;

    //
    // 📖 std::io
    // 📚 the trait Seek uses the
    //
    enum SeekFrom {
       Start(u64),  // destination = 0 + u64
       End(i64),    // destination = size + i64
       Current(i64) // destination = current pos + i64
    //
    fn seek(&mut self, pos: SeekFrom) ->  Result<u64>;


```

### ByteRecord

```rust
#[inline]
    pub fn new() -> ByteRecord {
        ByteRecord::with_capacity(0, 0)
    }

    #[inline]
    pub fn with_capacity(buffer: usize, fields: usize) -> ByteRecord {
        ByteRecord(Box::new(ByteRecordInner {
            pos: None,
            fields: vec![0; buffer],
            bounds: Bounds::with_capacity(fields),
        }))
    }
```

### Position of a record
```rust
    #[inline]
    pub fn position(&self) -> Option<&Position> {
        self.0.pos.as_ref()
    }

    #[derive(Clone, Eq)]
    pub struct ByteRecord(Box<ByteRecordInner>);

    // where self.0.pos.as_ref()
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ByteRecordInner {
        /// The position of this byte record.
        pos: Option<Position>,

        /// All fields in this record, stored contiguously.
        fields: Vec<u8>,

        /// The number of and location of each field in this record.
        bounds: Bounds,
    }

    pub struct Position {
        byte: u64,
        line: u64,
        record: u64,
    }

    struct Bounds {
        /// The ending index of each field.
        ends: Vec<usize>,
        /// The number of fields in this record.
        len: usize,
    }
```


## On the design
[source that describes design process](https://github.com/rust-lang/packed_simd/issues/65)

Technically, none of the APIs here are needed, since they are all implemented on top of ptr::{read,write}{_unaligned}. Their purpose is not to allow performing all the operations that the ptr methods allow, but to simplify reading and writing the portable vector types from/to slices.

The advantages for the safe methods are more clear because they are safe as opposed to the pointer methods. However, the checks that make them safe do incur a cost that is often not acceptable when writing SIMD code. The _unchecked variants provide an easy way to disable these checks without having to fall back to using the raw ptr APIs.

Also, even while one is dealing with raw pointers, it is still often more useful to create an slice from them if the memory is initialized and properly aligned, and then use these APIs, then doing the type punning required to map the portable vector types to arrays, casting that back to pointers, and so on.



#### Terminology - accessing memory

The "one base pointer, vector of offsets" approach to gathers is salvagable but requires a lower level approach -- in rust terms, taking a raw pointer base and a vector of offsets, and then each lane reads base.wrapping_offset(offsets.element(i)) or something like that.

Alternatively, I'd suggest the "vector of pointers" approach.


#### Sample for array to vector
[link to aligned vectors](https://github.com/ralfbiedert/simd_aligned_rust)

```rust
//Note: easy to &[f32x4] -> &[f32]

fn try_as_f32x4(x: &mut [f32]) -> Option<&mut [f32x4]> {
    if x.len() % f32x4::lanes() != 0 || x.as_ptr().align_offset(mem::align_of::<f32x4>()) != 0 {
       None
    } else {
        // safe because a `f32x4` is layout compatible with `[f32; 4],
        // each [f32; 4] within the [f32] slice is at mem::align_of::<f32x4>
        // and x.len() % f32x4::lanes() == 0
        unsafe {
                Some(slice::from_raw_parts_mut(x.as_ptr_mut() as *mut f32x4,
                        x.len() / f32x4::lanes()))
        }
    }
}
```

#### Iteration support from_slice_
```rust
fn compute_inner_kernel_simdf32x8(sv: &[f32], feature: &[f32], gamma: f32) -> f64 {
    type f32s = f32x8;

    let width = f32s::lanes();
    let steps = sv.len() / width;

    let mut sum = f32s::splat(0.0);

    for i in 0..steps {
        // When benchmarking `csvm_predict_sv1024_attr1024_problems1` with AVX2:

        // 238,928 ns / iter
        let a = unsafe { f32s::from_slice_aligned_unchecked(&sv[i * width..]) };
        let b = unsafe { f32s::from_slice_aligned_unchecked(&feature[i * width..]) };

        // 237,541 ns / iter
        // let a = unsafe { f32s::from_slice_unaligned_unchecked(&sv[i * width..]) };
        // let b = unsafe { f32s::from_slice_unaligned_unchecked(&feature[i * width..]) };

        // 343,970 ns / iter
        // let a = f32s::from_slice_aligned(&sv[i * width..]);
        // let b = f32s::from_slice_aligned(&feature[i * width..]);

        // 363,796 ns / iter
        // let a = f32s::from_slice_unaligned(&sv[i * width..]);
        // let b = f32s::from_slice_unaligned(&feature[i * width..]);

        // Add result
        sum += (a - b) * (a - b);
    }

    f64::from((-gamma * sum.sum()).exp())
}
  ```

#### Syntax to be aware of
You can check alignment requirements of various types using
```rust
std::mem::align_of::<T>() function.

// convert types, with proper alignment
[T]::align_to::<U>
```

```rust
// #[repr(C)]
#[repr(align(16))]
struct MyU8(u8);

fn main() {
    dbg!(size_of::<[MyU8; 10]>());
}
```

#### packed_simd notes

`f32x4` is an alias for `Simd<[f32; 4]>`
Vectors of pointers:: `Simd<[*const *mut f32; 4]>`
`From<[T; N]> for Simd<[T; N]>`

Syntax change: `Simd<T, N>` instead of `Simd<[T; N]>`


#### Vector functions

Gather
Scatter
permute! shuffle!
rotate s
splat: Load a single element and splat to all lanes

lane-wise casts: `.cast()` is ~ to as per vector lane.

#### Vector denotation

`{i,u,f,m}{lane_width}x{#lanes}`

For instance,
`i64x8` = 512-bit vector with 8 `i64` lanes.
  * the lane widths are each 64-bits

`f32x4` = 128-bit vector with 4 `f32` lanes.
  * the lane widths are each 32-bits

Masks
`m8x4` is a 32-bit vector mask with 4 lanes
  * the lane widths are each 8-bits
      * ✅ 00000000_11111111_00000000_11111111
      * 🚫 00000000_11111111_00000000_11101011


##### Slow
```rust
/// Computes the arithmetic average of the elements in the list.
///
/// # Panics
///
/// If `xs.len()` is not a multiple of `8`.
fn average_slow256(xs: &[f32]) -> f32 {
    // The 256-bit wide floating-point vector type is f32x8. To
    // avoid handling extra elements in this example we just panic.
    assert!(xs.len() % 8 == 0,
            "input length `{}` is not a multiple of 8",
            xs.len());

    let mut result = 0._f32;  // This is where we store the result

    // We iterate over the input slice with a step of `8` elements:
    for i in (0..xs.len()).step_by(8) {
        // First, we read the next `8` elements into an `f32x8`.
        // Since we haven't checked whether the input slice
        // is aligned to the alignment of `f32x8`, we perform
        // an unaligned memory read.
        let data = f32x8::read_unaligned(&xs[i..]);

        // With the element in the vector, we perform an horizontal reduction
        // and add them to the result.
        result += data.sum();
    }
    result / xs.len()
}
```

##### Faster...
… issue remains of the decrease in performance not using memory that is aligned with
the size of the vector.

```rust
fn average_fast256(xs: &[f32]) -> f32 {
    assert!(xs.len() % 8 == 0,
            "input length `{}` is not a multiple of 8",
            xs.len());

    // Our temporary result is now a f32x8 vector:
    let mut result = f32x8::splat(0.);
    for i in (0..xs.len()).step_by(8) {
        let data = f32x8::read_unaligned(&xs[i..]);
        // This adds the data elements to tour temporary result using
        // a vertical lane-wise simd operation - this is a single SIMD
        // instruction on most architectures.
        result += data;
    }
    // Perform a single horizontal reduction at the end:
    result.sum() / xs.len()
}
```

### Alignment

`bytes.align_to::<__128>`

```rust
fn my_algorithm(bytes: &[u8]) {
    unsafe {
        let (prefix, simd, suffix) = bytes.align_to::<__m128>();
        less_fast_algorithm_for_bytes(prefix);
        more_fast_simd_algorithm(simd);
        less_fast_algorithm_for_bytes(suffix);
    }
}
```

Returns three slices from 2 split locations:

  1. `input_slice[..r]`
  2. lcm( size_of::<T>(), size_of::<U>() )

  The location of the first split relies on `pointer::align_offset()`. The address p, advanced by (s * o) bytes, must be aligned to a. Note: 0 mod a, is always zero.

    p + (s * o) ≡ 0 mod a

    p := address of the first element in the input slice
    a := alignment of the output type U
    s := stride of type T
    o := offset in units of T to aligned value of U

Solving for o involves evaluating the following.  The solution is not unique and requires filtration: positive and smallest.

    a - (p mod a)           s
    --------------  x (( -------- ) ^ -1 mod a)
    gcd(s,a)             gcd(s,a)

The derivation is complicated and computationally expensive.  The use of heuristics are required to make it worth it! We know:

  1. alignments are  power of 2
  2. a / (2^k) ~ a » k
  3. a mod 2^k ~ a ^ (2^k - 1)
     * 2^k - 1 ~ 1111.. k bits
  4. gcd(a,b) -> where each is some 2^k
     * 0100 -> consecutive leading zeroes: 2^2

    let k = a.trailing_zeros().min(b.trailing_zeros());
    let gcd = 1 << k;
    let lcd = (a * b) >> k;

### Casting advice
You'd need to initialize the `Vec<u32>`, either by using the resize method or by initializing it up-front with `vec![0u32; size]`. Then you can take slices of it, cast them to `[u8]`, and copy into them with `copy_from_slice`.

Or if initialization proves too expensive, you can use raw pointers to write to the uninitialized part of the vector, and then `set_len` after it is initialized.

You could write your own vec-like structure, or make a `Vec<BigAlign>`, but you can't make std's `Vec<u8>` use a different alignment. Contract between `Vec` and the allocator is that the alignment will be default for the type, and won't change.

### The design of processing input fast

That is, logically the API call partitions the input buffer into a read head and an unread tail and the output buffer to a written head and an unwritten tail. It is easy to call the API again with the tails as the new buffers.

Another approach that’s essentially isomorphic would be fn foo(src: &mut &[u8], dist: &mut &mut [u8]), which rewrites the modifications to those pointers (closer to the C version). This alternate version is possibly a bit nicer to use in loops, since it doesn’t require manually reassigning variables.

```rust
fn foo<'a, 'b>(src: &'a [u8], dst: &'b mut [u8]) -> (&'a [u8], &'a [u8], &'b mut [u8], &'b mut [u8]);

fn foo(src: &mut &[u8], dist: &mut &mut [u8]) {}
```
The `Size` trait informs the size of a slice, but does not say anything about how the contents came from a much larger source. Said differently, `Sizes` says nothing to inform that they are views into the same large buffer. One could make a slice type that knows the size of the buffer it came from, exposing the operations you want.

```rust
fn foo(src: &mut &[u8], dst: &mut &mut [u8]) { ... }

fn bar(src: &[u8], dst: &mut [u8]) {
    let remaining_dst_len;
    {  // Limit the scope of remaining_* borrows
        let remaining_src = &src[..];
        let remaining_dst = &mut dst[..];
        while something {
            foo(&mut src, &mut dst)
        }
        remaining_dst_len = remaining_dst.len();
    }
    let written = &mut dst[..dst.len() - remaining_dst_len];
    ...
}

fn subslice_offset(&self, inner: &str) -> usize {
    let a_start = self.as_ptr() as usize;
    let a_end = a_start + self.len();
    let b_start = inner.as_ptr() as usize;
    let b_end = b_start + inner.len();

    assert!(a_start <= b_start);
    assert!(b_end <= a_end);
    b_start - a_start
}

// have function return usize to compute what was read/written
let src: *mut *const u8 = ...;
let src_end: *const u8 = ...;

let len = *src as usize - src_end as usize;
let slice = std::slice::from_raw_parts(*src, len);

// or the reverse
let slice: &[u8] = ...;

let mut ptr = slice.as_ptr();
let src_end = ptr.offset(slice.len());
let src = &mut ptr;
```
Since Rust wants to work with array indices and lengths instead of incremented pointers and sentinel pointers, I think I should just go with returning the number of code units read and written.

```rust
enum DecoderResult {
   Overflow,
   Underflow,
   Malformed,
}

#[no_mangle]
pub extern fn Decoder_decode_to_utf16(decoder: &mut Decoder, src: *const u8, src_len: *mut usize, dst: *mut u16, dst_len: *mut usize, last: bool) -> DecoderResult {
    let src_slice = unsafe { std::slice::from_raw_parts(src, *src_len) };
    let dst_slice = unsafe { std::slice::from_raw_parts_mut(dst, *dst_len) };
    let (result, read, written) = decoder.decode_to_utf16(src_slice, dst_slice, last);
    unsafe {
        *src_len = read;
        *dst_len = written;
    }
    result
}
#[no_mangle]
pub extern fn decoder_decode_to_utf16(decoder: &mut Decoder, src: *const u8, src_len: *mut usize, dst: *mut u16, dst_len: *mut usize, last: bool) -> DecoderResult {
    let src_slice = unsafe { std::slice::from_raw_parts(src, *src_len) };
    let dst_slice = unsafe { std::slice::from_raw_parts_mut(dst, *dst_len) };
    let (result, read, written) = decoder.decode_to_utf16(src_slice, dst_slice, last);
    unsafe {
        *src_len = read;
        *dst_len = written;
    }
    result
}
trait UtfUnit {}

impl UtfUnit for u8 {}

impl UtfUnit for u16 {}

trait Decoder {
    fn decode_to_utf16(&mut self, src: &[u8], dst: &mut [u16], last: bool) -> (DecoderResult, usize, usize);

    fn decode_to_utf8(&mut self, src: &[u8], dst: &mut [u8], last: bool) -> (DecoderResult, usize, usize);

    fn decode_to_str(&mut self, src: &[u8], dst: &mut str, last: bool) -> (DecoderResult, usize, usize) {
        let bytes: &mut [u8] = unsafe { std::mem::transmute(dst) };
        let (result, read, written) = self.decode_to_utf8(src, bytes, last);
        let len = bytes.len();
        let mut trail = written;
        while trail < len && ((bytes[trail] & 0xC0) == 0x80) {
            bytes[trail] = 0;
            trail += 1;
        }
        (result, read, written)
    }

    fn decode_to_string(&mut self, src: &[u8], dst: &mut String, last: bool) -> (DecoderResult, usize) {
        unsafe {
            let vec = dst.as_mut_vec();
            let old_len = vec.len();
            let capacity = vec.capacity();
            vec.set_len(capacity);
            let (result, read, written) = self.decode_to_utf8(src, &mut vec[old_len..], last);
            vec.set_len(old_len + written);
            (result, read)
        }
    }

    fn decode(&mut self, src: &[u8], dst: &mut [T], last: bool) -> (DecoderResult, usize, usize);
}
```

### The code that was used

```rust
//here
/// # Infinite loops
///
/// When converting with a fixed-size output buffer whose size is too small to
/// accommodate one character or (when applicable) one numeric character
/// reference of output, an infinite loop ensues. When converting with a
/// fixed-size output buffer, it generally makes sense to make the buffer
/// fairly large (e.g. couple of kilobytes).
pub struct Decoder {
    encoding: &'static Encoding,
    variant: VariantDecoder,
    life_cycle: DecoderLifeCycle,
}

impl Decoder {
    fn new(enc: &'static Encoding, decoder: VariantDecoder, sniffing: BomHandling) -> Decoder {
        Decoder {
            encoding: enc,
            variant: decoder,
            life_cycle: match sniffing {
                BomHandling::Off => DecoderLifeCycle::Converting,
                BomHandling::Sniff => DecoderLifeCycle::AtStart,
                BomHandling::Remove => {
                    if enc == UTF_8 {
                        DecoderLifeCycle::AtUtf8Start
                    } else if enc == UTF_16BE {
                        DecoderLifeCycle::AtUtf16BeStart
                    } else if enc == UTF_16LE {
                        DecoderLifeCycle::AtUtf16LeStart
                    } else {
                        DecoderLifeCycle::Converting
                    }
                }
            },
        }
    }
    /// Query the worst-case UTF-8 output size _with replacement_.
    ///
    /// Returns the size of the output buffer in UTF-8 code units (`u8`)
    /// that will not overflow given the current state of the decoder and
    /// `byte_length` number of additional input bytes when decoding with
    /// errors handled by outputting a REPLACEMENT CHARACTER for each malformed
    /// sequence or `None` if `usize` would overflow.
    ///
    /// Available via the C wrapper.
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        // Need to consider a) the decoder morphing due to the BOM and b) a partial
        // BOM getting pushed to the underlying decoder.
        match self.life_cycle {
            DecoderLifeCycle::Converting
            | DecoderLifeCycle::AtUtf8Start
            | DecoderLifeCycle::AtUtf16LeStart
            | DecoderLifeCycle::AtUtf16BeStart => {
                return self.variant.max_utf8_buffer_length(byte_length);
            }
            DecoderLifeCycle::AtStart => {
                if let Some(utf8_bom) = checked_add(3, byte_length.checked_mul(3)) {
                    if let Some(utf16_bom) = checked_add(
                        1,
                        checked_mul(3, checked_div(byte_length.checked_add(1), 2)),
                    ) {
                        let utf_bom = std::cmp::max(utf8_bom, utf16_bom);
                        let encoding = self.encoding();
                        if encoding == UTF_8 || encoding == UTF_16LE || encoding == UTF_16BE {
                            // No need to consider the internal state of the underlying decoder,
                            // because it is at start, because no data has reached it yet.
                            return Some(utf_bom);
                        } else if let Some(non_bom) =
                            self.variant.max_utf8_buffer_length(byte_length)
                        {
                            return Some(std::cmp::max(utf_bom, non_bom));
                        }
                    }
                }
            }
            DecoderLifeCycle::SeenUtf8First | DecoderLifeCycle::SeenUtf8Second => {
                // Add two bytes even when only one byte has been seen,
                // because the one byte can become a lead byte in multibyte
                // decoders, but only after the decoder has been queried
                // for max length, so the decoder's own logic for adding
                // one for a pending lead cannot work.
                if let Some(sum) = byte_length.checked_add(2) {
                    if let Some(utf8_bom) = checked_add(3, sum.checked_mul(3)) {
                        if self.encoding() == UTF_8 {
                            // No need to consider the internal state of the underlying decoder,
                            // because it is at start, because no data has reached it yet.
                            return Some(utf8_bom);
                        } else if let Some(non_bom) = self.variant.max_utf8_buffer_length(sum) {
                            return Some(std::cmp::max(utf8_bom, non_bom));
                        }
                    }
                }
            }
            DecoderLifeCycle::ConvertingWithPendingBB => {
                if let Some(sum) = byte_length.checked_add(2) {
                    return self.variant.max_utf8_buffer_length(sum);
                }
            }
            DecoderLifeCycle::SeenUtf16LeFirst | DecoderLifeCycle::SeenUtf16BeFirst => {
                // Add two bytes even when only one byte has been seen,
                // because the one byte can become a lead byte in multibyte
                // decoders, but only after the decoder has been queried
                // for max length, so the decoder's own logic for adding
                // one for a pending lead cannot work.
                if let Some(sum) = byte_length.checked_add(2) {
                    if let Some(utf16_bom) =
                        checked_add(1, checked_mul(3, checked_div(sum.checked_add(1), 2)))
                    {
                        let encoding = self.encoding();
                        if encoding == UTF_16LE || encoding == UTF_16BE {
                            // No need to consider the internal state of the underlying decoder,
                            // because it is at start, because no data has reached it yet.
                            return Some(utf16_bom);
                        } else if let Some(non_bom) = self.variant.max_utf8_buffer_length(sum) {
                            return Some(std::cmp::max(utf16_bom, non_bom));
                        }
                    }
                }
            }
            DecoderLifeCycle::Finished => panic!("Must not use a decoder that has finished."),
        }
        None
    }

/// Incrementally decode a byte stream into UTF-8 with malformed sequences
    /// replaced with the REPLACEMENT CHARACTER with type system signaling
    /// of UTF-8 validity.
    ///
    /// This methods calls `decode_to_utf8` and then zeroes
    /// out up to three bytes that aren't logically part of the write in order
    /// to retain the UTF-8 validity even for the unwritten part of the buffer.
    ///
    /// See the documentation of the struct for documentation for `decode_*`
    /// methods collectively.
    ///
    /// Available to Rust only.
    pub fn decode_to_str(
        &mut self,
        src: &[u8],
        dst: &mut str,
        last: bool,
    ) -> (CoderResult, usize, usize, bool) {
        let bytes: &mut [u8] = unsafe { dst.as_bytes_mut() };
        let (result, read, written, replaced) = self.decode_to_utf8(src, bytes, last);
        let len = bytes.len();
        let mut trail = written;
        // Non-UTF-8 ASCII-compatible decoders may write up to `MAX_STRIDE_SIZE`
        // bytes of trailing garbage. No need to optimize non-ASCII-compatible
        // encodings to avoid overwriting here.
        if self.encoding != UTF_8 {
            let max = std::cmp::min(len, trail + ascii::MAX_STRIDE_SIZE);
            while trail < max {
                bytes[trail] = 0;
                trail += 1;
            }
        }
        while trail < len && ((bytes[trail] & 0xC0) == 0x80) {
            bytes[trail] = 0;
            trail += 1;
        }
        (result, read, written, replaced)
    }
    /// Incrementally decode a byte stream into UTF-8 with type system signaling
    /// of UTF-8 validity.
    ///
    /// This methods calls `decode_to_utf8` and then zeroes out up to three
    /// bytes that aren't logically part of the write in order to retain the
    /// UTF-8 validity even for the unwritten part of the buffer.
    ///
    /// See the documentation of the struct for documentation for `decode_*`
    /// methods collectively.
    ///
    /// Available to Rust only.
    pub fn decode_to_str_without_replacement(
        &mut self,
        src: &[u8],
        dst: &mut str,
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        let bytes: &mut [u8] = unsafe { dst.as_bytes_mut() };
        let (result, read, written) = self.decode_to_utf8_without_replacement(src, bytes, last);
        let len = bytes.len();
        let mut trail = written;
        // Non-UTF-8 ASCII-compatible decoders may write up to `MAX_STRIDE_SIZE`
        // bytes of trailing garbage. No need to optimize non-ASCII-compatible
        // encodings to avoid overwriting here.
        if self.encoding != UTF_8 {
            let max = std::cmp::min(len, trail + ascii::MAX_STRIDE_SIZE);
            while trail < max {
                bytes[trail] = 0;
                trail += 1;
            }
        }
        while trail < len && ((bytes[trail] & 0xC0) == 0x80) {
            bytes[trail] = 0;
            trail += 1;
        }
        (result, read, written)
    }
```

### Memory alignment - the problem

```rust
struct {
  a: char;
  b: i32; // word
  c: i16; // short
}```

 on a 32-bit machine:

 -----
 | a | 0x0000
 |   |
 |   |
 |   |
 -----
 | b | 0x0004
 | b |
 | b |
 | b |
 -----
 | c | 0x0008
 | c |
 -----
 … copied over the network
  -----
 | a | 0x0000
 -----
 | b | 0x0001 🚫
 | b |
 | b |
 | b |
 -----
 | c | 0x0005 🚫
 | c |
 -----

 Because pointers only point to every 32-bits in memory, if the location of the information is "off" the mark, there is extra work to do to "focus" the result from the pointer.

 If the system can assume that the two last bits (2 LSB) are always zero, it can access a larger memory range for a given approach. e.g., on a 32-bit machine,

     0x0000 « implies a 1-byte stride
     0x00xx « implies a 4-byte stride

 To read c, the processor must read a word starting at 0x0004, then shift left 1 to get it in the 16-bit register.
   * read 0x0004 (=> bccx) into the result register
   * left shift => ccx0
   * read with a 16-bit register

 To read b from 0x0001
   * read 0x0000 into the result register
   * shift left 1 byte (abbb => bbb0)
   * read 0x0004 into the tmp register (=> bccx)
   * shift right 3 bytes (bccx => 000b)
   * combine with or: bbb0 | 000b => bbbb


### Memory on a x86 chip

The physical address is generated by multiplying the segment register by 16, then adding a 16-bit offset. Using 16-bit offsets implicitly limits the CPU to 64k segment sizes (2^16 = ~64k). Some programmers have programmed around this 64k segment size limitation by incrementing the contents of the segment registers. Their programs can point to 64k segments in 16-byte increments.

The descriptor cache registers contain information defining the segment base address, segment size limit, and segment access attributes, and are used for all memory references -- regardless of the values in the segment registers.

Format: Segment:Offset

    BS:0000
    BS:0004
    BS:0008
    ..
    BS:03FC
    etc..

  * Physical address is coded using something we don't see (segment descriptor cache)
  * Memory we see is made by adding the 16 or 32-bit offset to the base address in the above mentioned cache
  * The memory we see is used to compute the physical location of the memory (the hidden stuff); the lookup tables are set dynamically and thus the means for computing the memory.
  * The base address is not known until runtime; despite static values being recorded

### Converting between vector types
The layout of a vector is compatible with a fixed-sized array.
```rust
[f32; 4] ~ f32x4

// such that
impl From<[E; N]> for ExN;
impl From<ExN> for [E; N];
```

There are three ways to do so

##### `From/Into`

  * widen a lane (note the single direction)
  * lane count remains constant
  * vector size increases
  * e.g., `f32x4` -> `f64x4`

##### `as` (no impact on endien)

  * narrow a lane size
  * lane count remains constant
  * vector size decreases
  * e.g., `f32x4` -> `f8x4`

#### `transmute` (impacts endien)

  * change the lane size
  * change the lane count
  * vector size remains constant
  * e.g., `f32x4` -> `f64x2`

### ⚠️  Pass through memory

This example will pass the variable `a` through memory.  The caller of `foo` will place `a` on the stack.  `foo` will read `a` from the stack.  In the event `foo` changes `a`, the caller is unaware. They're semantically pass-by-value but implemented as pass-via-pointers.

```rust
fn foo(a: u32x4) { /* ... */ }
foo(u32x4::splat(3));

```

### Register kinds
There are different kinds of registers.  When moving between them, alignment is most likely required to get the expected performance.

A generic approach is to use `#[repr(simd)]`; the issue is that it is not portable.  Instead use `mem::align_of`?

  * Memory
  * Vector

### Likely often used
[Rust SIMD Performance Guide](https://rust-lang.github.io/packed_simd/perf-guide/bound_checks.html)

Reading and writing packed_vectors is checked; the slice is big enough, the slice is suitably aligned to store the Simd. The default checks reduce performance.

```rust

    Simd<[T; N]>::from_slice_aligned(& s[..])
    Simd<[T; N]>::write_to_slice_aligned(&mut s[..])

```

### The inner loop is where the work takes place
This is where we want to *avoid* using horizontal operations, and instead use vertical
vector operations.

### The Vec pointer layout

```rust
{starting_address: usize, allocated_storage_size: usize, current_vec_length: usize}
```

### Memory layout in regular Rust

```rust
#[derive(Debug)]
struct Foo {
    bar: Baz,
}

#[derive(Debug)]
struct Baz (
    Quux,
);

#[derive(Debug)]
struct Quux (
    Zom,
);

#[derive(Debug)]
struct Zom (
    u8,
    Nef,
);

#[derive(Debug)]
struct Nef (
    Quibble,
);

#[derive(Debug)]
struct Quibble (
    i32,
);

macro_rules! raw_dbg {(
    $expr:expr
) => (match $expr { expr => {
    eprintln!("{:#?} = {:#x?}", expr, unsafe {
        ::core::slice::from_raw_parts(
            &expr as *const _ as *const ::std::sync::atomic::AtomicU8,
            ::core::mem::size_of_val(&expr),
        )
    });
    expr
}})}

fn main ()
{
    raw_dbg!(Foo {
        bar: raw_dbg!(Baz(
            raw_dbg!(Quux(
                raw_dbg!(Zom(
                    raw_dbg!(1),
                    raw_dbg!(Nef(
                        raw_dbg!(Quibble(
                            raw_dbg!(5),
                        )),
                    )),
                )),
            )),
        )),
    });
}
```

## Starting for someone tackling a similar problem

```rust
pub enum DecoderResult {
   Overflow,
   Underflow,
   Malformed,
}

#[no_mangle]
pub extern fn decoder_decode_to_utf16(
    decoder: &mut Decoder,
    src: *const u8,
    src_len: *mut usize,
    dst: *mut u16,
    dst_len: *mut usize,
    last: bool) -> DecoderResult {

    let src_slice = unsafe { std::slice::from_raw_parts(src, *src_len) };

    let dst_slice = unsafe { std::slice::from_raw_parts_mut(dst, *dst_len) };

    let (result, read, written) = decoder.decode_to_utf16(src_slice, dst_slice, last);

    unsafe {
        *src_len = read;
        *dst_len = written;
    }

    result
}

trait UtfUnit {}

impl UtfUnit for u8 {}

impl UtfUnit for u16 {}

pub trait Decoder {
    fn decode_to_utf16(&mut self, src: &[u8], dst: &mut [u16], last: bool) -> (DecoderResult, usize, usize);

    fn decode_to_utf8(&mut self, src: &[u8], dst: &mut [u8], last: bool) -> (DecoderResult, usize, usize);

}

struct FooDecoder {
    // ...
}

impl FooDecoder {
    fn decode<T: UtfUnit>(&mut self, src: &[u8], dst: &mut [T], last: bool) -> (DecoderResult, usize, usize) {
        // ...
    }
}

// copypasta for each Decoder implementation
impl Decoder for FooDecoder {
    fn decode_to_utf16(&mut self, src: &[u8], dst: &mut [u16], last: bool) -> (DecoderResult, usize, usize) {
        self.decode(src, dst, last)
    }

    fn decode_to_utf8(&mut self, src: &[u8], dst: &mut [u8], last: bool) -> (DecoderResult, usize, usize) {
        self.decode(src, dst, last)
    }
}
```

### std split function
📖 core::str::mod

✨
👉 what we match on to split *is a* pattern

👉 a Pattern is a trait that defines `into_searcher`

👉 a Searcher is a trait that defines
   * A getter for the underlying string to be searched `fn haystack(&self) ->  &'a str;`
   * `fn next(&mut self) ->  SearchStep;`

👉 a SearchStep is an enum to describe the outcome of the search
   * `Match(usize, usize)`
   * `Reject(usize, usize)`
   * `Done`

```rust
#[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split<'a, P: Pattern<'a>>(&'a self, pat: P) -> Split<'a, P> {
        Split(SplitInternal {
            start: 0,
            end: self.len(),
            matcher: pat.into_searcher(self),
            allow_trailing_empty: true,
            finished: false,
        })
    }
```

### Pointer Math

📖 rust/src/libstd/path.rs

```rust
  let end_file_stem = file_stem[file_stem.len()..].as_ptr() as usize;
  let start = os_str_as_u8_slice(&self.inner).as_ptr() as usize;
  let v = self.as_mut_vec();
  v.truncate(end_file_stem.wrapping_sub(start));

  fn _set_extension(&mut self, extension: &OsStr) -> bool {
        let file_stem = match self.file_stem() {
            None => return false,
            Some(f) => os_str_as_u8_slice(f),
        };

        // truncate until right after the file stem
        let end_file_stem = file_stem[file_stem.len()..].as_ptr() as usize;
        let start = os_str_as_u8_slice(&self.inner).as_ptr() as usize;
        let v = self.as_mut_vec();
        v.truncate(end_file_stem.wrapping_sub(start));

        // add the new extension, if any
        let new = os_str_as_u8_slice(extension);
        if !new.is_empty() {
            v.reserve_exact(new.len() + 1);
            v.push(b'.');
            v.extend_from_slice(new);
        }

        true
    }
```

### Record terminator

```rust
#[derive(Clone, Copy, Debug)]
pub enum Terminator {
    /// Parses `\r`, `\n` or `\r\n` as a single record terminator.
    CRLF,
    /// Parses the byte given as a record terminator.
    Any(u8),
    /// Hints that destructuring should not be exhaustive.
    ///
    /// This enum may grow additional variants, so this makes sure clients
    /// don't count on exhaustive matching. (Otherwise, adding a new variant
    /// could break existing code.)
    #[doc(hidden)]
    __Nonexhaustive,
}

impl Terminator {
    /// Convert this to the csv_core type of the same name.
    fn to_core(self) -> csv_core::Terminator {
        match self {
            Terminator::CRLF => csv_core::Terminator::CRLF,
            Terminator::Any(b) => csv_core::Terminator::Any(b),
            _ => unreachable!(),
        }
    }
}

impl Default for Terminator {
    fn default() -> Terminator {
        Terminator::CRLF
    }
}
```

In core_csv

```rust
/// A record terminator.
///
/// Use this to specify the record terminator while parsing CSV. The default is
/// CRLF, which treats `\r`, `\n` or `\r\n` as a single record terminator.
#[derive(Clone, Copy, Debug)]
pub enum Terminator {
    /// Parses `\r`, `\n` or `\r\n` as a single record terminator.
    CRLF,
    /// Parses the byte given as a record terminator.
    Any(u8),
    /// Hints that destructuring should not be exhaustive.
    ///
    /// This enum may grow additional variants, so this makes sure clients
    /// don't count on exhaustive matching. (Otherwise, adding a new variant
    /// could break existing code.)
    #[doc(hidden)]
    __Nonexhaustive,
}

impl Terminator {
    /// Checks whether the terminator is set to CRLF.
    fn is_crlf(&self) -> bool {
        match *self {
            Terminator::CRLF => true,
            Terminator::Any(_) => false,
            _ => unreachable!(),
        }
    }

    fn equals(&self, other: u8) -> bool {
        match *self {
            Terminator::CRLF => other == b'\r' || other == b'\n',
            Terminator::Any(b) => other == b,
            _ => unreachable!(),
        }
    }
}

impl Default for Terminator {
    fn default() -> Terminator {
        Terminator::CRLF
    }
}
```

### How to fill a record buffer with contents of...

📖 core::csv::Reader

`read_byte_record_impl`

Note the enumeration that considers the various states of the buffer.

```rust
  match res {
      InputEmpty => continue,
      OutputFull => {
          record.expand_fields();
          continue;
      }
      OutputEndsFull => {
          record.expand_ends();
          continue;
      }
      Record => {
          record.set_len(endlen);
          self.state.add_record(record)?;
          return Ok(true);
      }
      End => {
          self.state.eof = ReaderEofState::Eof;
          return Ok(false);
      }
 }
```

What generates the `res` variable being matched: `self.core.read_record`.
...in turn: `let input = input_res?`
...in turn: `let input_res = self.rdr.fill_buf();`

```rust
loop {

  let (res, nin, nout, nend) = {
     let input_res = self.rdr.fill_buf();
     if input_res.is_err() {
         self.state.eof = ReaderEofState::IOError;
     }
     let input = input_res?;
     let (fields, ends) = record.as_parts();
     self.core.read_record(
         input,
         &mut fields[outlen..],
         &mut ends[endlen..],
     )
  };
  // … where the loop's continuation is controlled by the above pattern match findings.
}
```

What is `self.rdr.fill_buff()`?
Self is `csv::Reader`.
The `self.rdr` is a `io::BufReader`

```rust
fn new(builder: &ReaderBuilder, rdr: R) -> Reader<R> {
    Reader {
            core: Box::new(builder.builder.build()),
            rdr: io::BufReader::with_capacity(builder.capacity, rdr), // ✨
            state: ReaderState {
                headers: None,
                has_headers: builder.has_headers,
                flexible: builder.flexible,
                trim: builder.trim,
                first_field_count: None,
                cur_pos: Position::new(),
                first: false,
                seeked: false,
                eof: ReaderEofState::NotEof,
            },
        }
```
