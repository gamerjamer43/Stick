use strum_macros::AsRefStr;

use std::fmt::{Display, Formatter, Result};

use crate::lexer::Token;
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

/// a generic error for anything that may happen during lexing.
#[derive(Debug, PartialEq, Clone, AsRefStr)]
pub enum LexError<'src> {
    UnterminatedString(&'src str),
    UnterminatedChar(&'src str),
    UnknownToken(&'src str),
}

/// a generic error for anything that may happen during parsing.
#[derive(Debug, PartialEq, Clone, AsRefStr)]
pub enum ParseError<'src> {
    // an expected token is missing
    MissingExpected(&'src str),

    // const is not allowed in tandem w this variable
    ConstDisallowed(&'src str),
}

/// semantic errors found during analysis and type checking
#[derive(Debug, PartialEq, Clone, AsRefStr)]
pub enum SemanticError<'src> {
    TypeInference(&'src str),
    TypeMismatch(&'src str),
    UnknownIdentifier(&'src str),
    ImmutableBinding(&'src str),
    InvalidOperation(&'src str),
    Overflow(&'src str),
}

/// unified place to hold any error that may happen during compile time
#[derive(Debug, PartialEq, Clone, Default, AsRefStr)]
pub enum SyntaxError<'src> {
    Lex(LexError<'src>),
    Parse(ParseError<'src>),
    Semantic(SemanticError<'src>),

    #[default]
    Unknown,
}

impl Display for LexError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        use LexError::*;
        match self {
            UnterminatedString(s) => write!(f, "{s} is missing a closing \""),
            UnterminatedChar(s) => write!(f, "{s} is missing a closing '"),
            UnknownToken(s) => write!(f, "the hanging symbol '{s}' is not in the language spec."),
        }
    }
}

impl Display for ParseError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        use ParseError::*;
        match self {
            MissingExpected(s) => write!(f, "missing a value where expected, {s}"),
            ConstDisallowed(s) => {
                write!(f, "const cannot be used with some modifiers: {s}")
            }
        }
    }
}

impl Display for SemanticError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        use SemanticError::*;
        match self {
            TypeInference(s) => write!(f, "failed to infer type for {s}"),
            UnknownIdentifier(s) => write!(f, "identifier hasnt been declared yet {s}"),
            ImmutableBinding(s) => write!(f, "cannot mutate immutable binding {s}"),
            Overflow(s) => write!(f, "potentially implement a bounds check! {s}"),
            TypeMismatch(s) | InvalidOperation(s) => write!(f, "{s}"),
        }
    }
}

impl Display for SyntaxError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            SyntaxError::Lex(le) => write!(f, "{le}"),
            SyntaxError::Parse(pe) => write!(f, "{pe}"),
            SyntaxError::Semantic(se) => write!(f, "{se}"),

            // catchall == unknown
            SyntaxError::Unknown => write!(
                f,
                "TODO: add context to unknown errors. this is going to be exhaustive but in the event we don't match..."
            ),
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
pub fn lex_err<'src>(lex: &mut Lexer<'src, Token<'src>>) -> SyntaxError<'src> {
    let slice: &str = lex.slice();
    match slice.as_bytes().first() {
        Some(b'"') => SyntaxError::Lex(LexError::UnterminatedString(slice)),
        Some(b'\'') => SyntaxError::Lex(LexError::UnterminatedChar(slice)),
        Some(_) => SyntaxError::Lex(LexError::UnknownToken(slice)),

        // catch all in case none matched
        None => SyntaxError::Unknown,
    }
}
