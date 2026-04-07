//! Parser plugin system for p4k data extraction.
//!
//! Each parser implementation handles a specific directory layout produced by
//! a particular extraction tool (scdatatools/StarFab, unp4k, etc.).

pub mod scdatatools;
pub mod unp4k;

use std::path::Path;

use anyhow::Result;

use crate::model::ParseResult;

/// Trait implemented by each parser plugin.
///
/// A parser knows how to detect whether a directory matches its expected layout
/// and how to walk the directory tree, extracting nodes and edges into a
/// [`ParseResult`].
pub trait P4kParser: Send + Sync {
    /// Human-readable name of this parser (e.g. "scdatatools").
    fn name(&self) -> &str;

    /// Return `true` if `path` looks like it was produced by this parser's
    /// extraction tool.
    fn detect(&self, path: &Path) -> bool;

    /// Parse the extracted data at `path` for the given game `version`.
    ///
    /// Individual file errors should be captured as [`crate::model::ParseWarning`]
    /// entries rather than causing the entire parse to fail.
    fn parse(&self, path: &Path, version: &str) -> Result<ParseResult>;
}

pub use scdatatools::ScdatatoolsParser;
pub use unp4k::Unp4kParser;

/// Return all available parser implementations.
pub fn all_parsers() -> Vec<Box<dyn P4kParser>> {
    vec![
        Box::new(ScdatatoolsParser),
        Box::new(Unp4kParser),
    ]
}

/// Auto-detect which parsers can handle the given `path`.
///
/// Returns all parsers whose `detect()` returns `true`, ordered by priority
/// (scdatatools first).
pub fn detect_parsers(path: &Path) -> Vec<Box<dyn P4kParser>> {
    all_parsers()
        .into_iter()
        .filter(|p| p.detect(path))
        .collect()
}
