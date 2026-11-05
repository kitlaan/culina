//! culina-core — parser, AST, and canonical printer for `.culina` recipes.
//!
//! Build plan (design.md): block grammar first, inline sigils next, then the
//! field grammar and DAG validation. Every phase ends with a round-trip on
//! the stress fixtures in `tests/fixtures/`.

pub mod amounts;
pub mod ast;
pub mod dag;
pub mod error;
pub mod fields;
pub mod fmt;
pub mod inline;
pub mod lexicon;
pub mod metadata;
pub mod parse;
pub mod print;
pub mod render;
pub mod units;

pub use error::{ParseError, Result};
