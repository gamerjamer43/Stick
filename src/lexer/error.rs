use std::fmt::{Display, Formatter, Result};

use super::token::Token;
use logos::Lexer;

/// i don't need this schlanging out but i don't wanna type it 20 times
#[macro_export]
macro_rules! usage {
    () => {
        eprintln!("usage:");
        eprintln!("cargo run (optional: --release) <file>\n");
        eprintln!("flags:");
        eprintln!("-d | --debug     = debug mode on, prints lexer and parser outputs, as well as time and some performance stats.");
        eprintln!("-ff | --fastfail = fail immediately on one syntax error instead of warning you of others.");
        exit(2);
    };

    // if something provided print it first (this macro allows for any formatting inside)
    ($($msg:tt)+) => {{
        eprintln!($($msg)*);
        usage!();
    }};
}

// originally borrowed from https://docs.rs/logos/latest/logos/struct.Lexer.html
/// a generic error for anything that may happen during lexing.
#[derive(Debug, PartialEq, Clone, Default)]
pub enum SyntaxError<'src> {
    UnterminatedString(&'src str),
    UnterminatedChar(&'src str),
    UnknownToken(&'src str),

    #[default]
    Unknown,
}

impl Display for SyntaxError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            // &&str confused me. but we borrow on lines 10/11, and because of that the match borrows again.
            // this shit just double referencing
            SyntaxError::UnterminatedString(s) => write!(
                f, "\x1b[1mUnterminatedString:\x1b[22m Strings must be properly terminated. Afflicted: {s}"
            ),
            SyntaxError::UnterminatedChar(s) => write!(
                f, "\x1b[1mUnterminatedChar:\x1b[22m Chars must be properly terminated. Afflicted: {s}"
            ),
            SyntaxError::UnknownToken(s) => write!(
                f, "\x1b[1mUnknownToken:\x1b[22m The character '{s}' is not in the grammar for this language."
            ),

            // catchall == unknown
            SyntaxError::Unknown => write!(f, "unknown lexer error"),
        }
    }
}

/// a general error callback that will be done on any error that happens in lexing,
/// pulling from SyntaxError (rn just untermed "" or '', but there may be a couple others).
/// most errors will throw in parsing, but some will have to throw here
///
/// # Basic Usage
/// when a lexer error occurs, this will also be fired with it (attached to Logos).
/// this will match an error to one of the following or unknown (default).
///
/// # Returns
/// an option from the SyntaxError enum if matched, or unknown by default
pub fn err<'src>(lex: &mut Lexer<'src, Token<'src>>) -> SyntaxError<'src> {
    let slice: &str = lex.slice();
    match slice.as_bytes().first() {
        Some(b'"') => SyntaxError::UnterminatedString(slice),
        Some(b'\'') => SyntaxError::UnterminatedChar(slice),
        Some(_) => SyntaxError::UnknownToken(slice),

        // catch all in case none matched
        None => SyntaxError::Unknown,
    }
}