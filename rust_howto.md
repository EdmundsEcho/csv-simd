
## this works, but not this

```rust
pub fn read(memmap: &Mmap) -> StructureIndex {
  //...
  let (head_u8, body_vector, tail_u8) = unsafe { (*memmap).align_to::<__m128>() };
  //                                              ^^^^^^^ value
  //...
}

std::str::from_utf8_unchecked(self.memmap[*mem_start..*mem_end]) })
  //                          ^^^^ value
std::str::from_utf8_unchecked(&self.memmap[*mem_start..*mem_end]) })
  //                          ^ must create a ref

```


The function does not have "self" in the parameter; the function, as I'm using it, is using self as a parameter. Difference because only functions with a reference to self cast self as needed to meet the spec.
```rust
pub unsafe fn from_utf8_unchecked(v: &[u8]) -> &str
```

This function is using the "self reference" and thus will cast self -> &self

```rust
pub unsafe fn align_to<U>(&self) -> (&[T], &[U], &[T])
```
