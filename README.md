# csv-simd
Adapted Lemir simdjson algorithm applied to build csv memory index.  Uses byte "nibles" to increase processing capacity to identify structure in a csv.

A. `csv -> memory index`
B. `record number, index -> record`
C. `record, field idx, index -> field value`

# Meta todos

1. set the open source licensing
2. describe the project
3. how to participate
4. use the BurntSushi csv parser as a benchmark

# Decisions

1. Documenting the core concepts (vs the specifics of the API)
2. Adding a dependency https://github.com/rust-lang/portable-simd
3. Extend the capability to streams (not all in memory as it is now)
4. Consider splitting work without first knowing record breaks (requires toggling interpretation if/when start in quoted text)


# Code todos

1. Make compatible for M1 (supports NEON)
2. Document the public api
3. Document the active tests and coverage
4. Take inventory of how to augment the compliance with the csv standard to include escape and commas within quoted text
