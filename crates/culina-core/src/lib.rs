//! culina-core — parser, AST, and canonical printer for `.culina` recipes.
//!
//! Build plan (design.md): block grammar first, inline sigils next, then the
//! field grammar and DAG validation. Every phase ends with a round-trip on
//! the stress fixtures in `tests/fixtures/`.

pub mod ast;
pub mod error;
pub mod parse;
pub mod print;

pub use error::{ParseError, Result};
