#![forbid(unsafe_code)]

// The minification logic lives in the published `sailfish-minify` crate
// (`src/minify.rs`). This crate only exists so the logic can be exercised by
// unit tests and benchmarks (a proc-macro crate cannot export non-macro items
// to integration tests), so it is never published.
#[path = "../../src/minify.rs"]
mod minify;

pub use minify::*;
