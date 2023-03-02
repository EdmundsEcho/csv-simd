//
// 📚 Using use brings the path into scope of what follows
//    ...it allows for a shorthand using the last token as the alias.
//
//    👉 crate means root
//    👉 self means this file
//    👉 idiomatic to have the alias = parent
//    👉 idiomatic struct/enum to use a fully qualified path
//

fn main() {
    // csv_simd::run().expect("Failed to parse the csv");
    println!("csv_simd::main - not required");
}
