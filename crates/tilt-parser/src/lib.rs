// ===================================================================
// FILE: lib.rs (tilt-parser crate)
//
// DESC: Main module for the TILT parser, using LALRPOP for parsing
//       and logos for lexing.
// ===================================================================

pub mod lexer;
pub mod tests;

// Include the generated LALRPOP parser
use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub tilt); // synthesizes the `tilt` module

// Re-export for convenience
pub use lexer::Token;
pub use tilt::*;
pub use tilt_ast::*;
