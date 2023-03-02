Goal:
1. use vectorized computation to identify structural components of a csv file.
2. use the result to run frequency counts, and function search
3. use vectorized computation to perform the function search

Overview of the components
1. Chunking the data for multi-thread analysis
2. Within each thread, vectorized analysis for multiple simultaneous computations

Structural vs non-structural keys
1. quote
2. comma
3. newline

Possible states when parsing (FYI, not applicable)
1. Record start (R)
2. Field start (F)
3. Unquoted field (U)
4. Quoted field (Q)
5. Quoted end (E)

Snippets
[rust simd-json] (https://github.com/simd-lite/simd-json)
[c++ simdjson] (https://github.com/simdjson/simdjson)
[pikkr based on Mison] (https://github.com/pikkr/pikkr)
[auto vec demo] (https://github.com/nickwilcox/autovec_demo/blob/master/src/lib.rs)
[lemir simdcsv in C] (https://github.com/geofflangdale/simdcsv)
[lemic blog code] (https://github.com/lemire/Code-used-on-Daniel-Lemire-s-blog)
[sparser: filter] (https://github.com/stanford-futuredata/sparser/tree/sparser-opensource/sparser)

Parsing tools
1. 16-byte lookup table
2. Identify varying sets of characters using the same two vpshufb instructions

Bitmaps
1. Number of sets align with number of bit indexes (8)
2. to differentiate values in the domain, we have to use an injective function
   -> the bitmap is a simplification of all possible values
   -> design: simplify the encoding wilst avoiding collisions

      code point
      0x2c                   = comma      -> 000001
      0x3a                   = collon     -> 000010
      0x5b, 0x5d, 0x7b, 0x7d = brackets   -> 000100
      0x09, 0x0a, 0x0d       = whitespace -> 001000
      0x20                   = space      -> 010000
      others                 = others     -> 100000

      0x5c                   = \          -> 100000


Bitmap for CSV

      structural code points
      0x0d 0x0a = end line -> 000001
      0x2c      = comma    -> 000010

      pseudo-structural code points
      0x20      = space    -> 000100
      0x22      = escape   -> 001000
      0x5c      = quote    -> 010000

3. process blocks of 64 input bytes -> 64-bit bitset

Computation
bitsets -> indexes
* count trailing zeroes
  * tzcnt instruction
* clear the lowest set bit: s = s & (s - 1) (blsr instruction)
  * avoid branch: extract 8 indexes then, ignore excessively extracted by overwriting them
    (don't advance the buffer index)

* ASCII test: most significant bit of all bytes = 0
* series of SIMD instructions to validate UTF8
  - vpshufb to map bytes to 0, 2, 3 and 4; ASCII to 1.

* Tracking error
  - 32-byte vector with zeroes
  - compute bitwise OR of the result of each check
  - once, at the end of the process if value is 0

Recipe Part 1
Identify 10 different values -> structural chars or whitespace
':', '\', '"', '{', '}', plus 4 whitespace (space, tab, new line)

1. identify escaped quotes; locate quote and backslash positions
2. use bitwise ANDNOT to eliminate the escaped quote characters
3. identify between quotes to identify structure
   - bit pattern 1 for odd-number of unescaped quotes
     0b100010000 for quote locations
     0b011110000 for locations between quotes
     - prefix sum of the XOR: bit-value i = cumulative XOR bit-value including i
       for (i=0, i<64; i++) { mask = mask xor (mask << 1) }
       pclmulqdq instruction carry-less multiplication with 64-bit word with all 1

4. vpshufb as a vectorized lookup: it uses the least significant 4 bits of each byte
   as an index into a 16-byte table.
   - domain = 16 byte, codomain = 1 byte
   So, one lookup followed by a 4-bit right shift and a second lookup of a different table
   we can classify: structural characters and white-space characters.
   - low-value nibble of each byte  -> 4 bit
   - high-value nibble of each byte -> 4 bit
   - bitwise AND                    -> single value

   The vpshufb instruction
   1. uses the least significant 4 bits of each byte as an index into a 16 byte table.
      -> byte value from the 16 byte table (the 16 byte table yields 1 byte using one of 16
      values encoded in the 4-bit low value nibble)

   2. then 4-bit right shift then a second lookup of a different table

   again, how identify sets of characters
                  low   high nibble
   e.g., 0x9 1001  9     0
         0xa 1010  a     0
         0xd 1101  d     0

         set             low   high
   e.g., 0x5b 0101 1011    b     5
         0x5d 0101 1101    d     5
         0x7b 0111 1011    b     7
         0x7d 0111 1101    d     7

         set             low   high
   e.g., 0x21 0010 0001    1     2
         0x33 0011 0011    3     3

Bitmaps for CSV

     first table
     [0,1, 2,3,4,5,6,7,8,9,a,b, c,d,e,f] << low nibble value of char (byte)
     [4,0,16,0,0,0,0,0,0,0,1,0,10,1,0,0] << encode

     ... shift 4 right

     first table
     [0,1, 2,3,4,5,6,7,8,9,a,b,c,d,e,f] << high nibble value of char (byte)
     [1,0,22,0,0,8,0,0,0,0,0,0,0,0,0,0] << encode


Index extraction

Review, process blocks of

  64 input bytes -> 64-bit bitset corresponding to structure/pseudo

Method: Count trailing zeroes using
👉 tzcnt instruction + clear lowest set bit: s = s & (s - 1) (blsr)


