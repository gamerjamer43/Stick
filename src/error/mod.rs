pub mod diagnostic;
pub mod errors;

pub use diagnostic::{Diagnostic, dump};
pub use errors::{ParseError, SemanticError, SyntaxError, lex_err};
