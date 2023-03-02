### Tasks

  1. `find_structura_bits(input: &[u8]) -> std::result::Result<Vec<u32>, ErrorType>`
     * take a ref to the u8 input

     1. Iterate on the input: iterate idx < lenminus64

         1. instantiate the SimdInput
         `let input = SimdInput::new(input.get_unchecked(idx as usize..))`

         1. input.check_utf8

         1. Create memory for the structural input
         `let mut structural_indexes = Vec::with_capacity(len / 6);`

         1. SimdInput::flatten_bits(&mut structural_indexes, idx as u32, structurals);

         1. increment idx SIMDINPUT_LENGTH;

    1. if idx < len perform the computation using a smaller buffer?

  ```rust
  pub fn from_slice_with_buffers(
        input: &'de mut [u8],
        input_buffer: &mut AlignedBuf,
        string_buffer: &mut [u8],
    ) -> Result<Self> {
        let len = input.len();
  ```



```rust

   pub fn from_slice_with_buffers(
        input: &'de mut [u8],
        input_buffer: &mut AlignedBuf,
        string_buffer: &mut [u8],
    ) -> Result<Self> {
        let len = input.len();

        if len > std::u32::MAX as usize {
            return Err(Deserializer::error(ErrorType::InputTooLarge));
        }

        if input_buffer.capacity() < len + SIMDJSON_PADDING * 2 {
            *input_buffer = AlignedBuf::with_capacity(len + SIMDJSON_PADDING * 2);
        }

        unsafe {
            input_buffer
                .as_mut_slice()
                .get_unchecked_mut(..len)
                .clone_from_slice(input);
            *(input_buffer.get_unchecked_mut(len)) = 0;
            input_buffer.set_len(len);
        };

        let s1_result: std::result::Result<Vec<u32>, ErrorType> =
            unsafe { Deserializer::find_structural_bits(&input_buffer) };

        let structural_indexes = match s1_result {
            Ok(i) => i,
            Err(t) => {
                return Err(Error::generic(t));
            }
        };

        let tape =
            Deserializer::build_tape(input, &input_buffer, string_buffer, &structural_indexes)?;

        Ok(Deserializer { tape, idx: 0 })
    }

```
