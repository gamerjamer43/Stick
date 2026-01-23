use super::{
    diagnostic::{Diagnostic, dump},
    token::Token,
};
use logos::{Lexer, Logos};
use std::{ops::Range, result::Result, time::Instant};

// would like guidance as to if i'm doing this cleanly or if i'm nesting too much
// i like the if let syntax frm ocaml carrying over. v heavy emphasis on pattern matching
// src is a horrible name its just the source file
pub fn lex<'path, 'src>(
    path: &'path str,
    src: &'src str,
    debug: bool,
    fastfail: bool,
) -> Result<Vec<Token<'src>>, Vec<Diagnostic<'path, 'src>>> {
    let mut errors: Vec<Diagnostic<'path, 'src>> = Vec::new();
    let mut tokens: Vec<Token<'src>> = Vec::new();
    let start: Instant = Instant::now();

    let linecount: usize = src.lines().count();
    let mut lex: Lexer<'_, Token> = Token::lexer(src);
    while let Some(res) = lex.next() {
        match res {
            Ok(tok) => {
                // print token info if debug is on
                if debug {
                    println!("[bytes {:?}]: {tok} ", lex.span());
                }
                tokens.push(tok)
            }

            // any errors have types in the SyntaxError enum, Unknown by default
            Err(err) => {
                let span: Range<usize> = lex.span();
                let diagnostic: Diagnostic<'_, '_> = Diagnostic {
                    path,
                    src,
                    span,
                    err,
                };
                errors.push(diagnostic);

                // fail immediately on ff
                if fastfail {
                    break;
                }
                continue;
            }
        };
    }

    // handle debug prints
    println!(
        "Lexed {} bytes, {linecount} lines into {} tokens. Took {}s.",
        src.len(),
        tokens.len(),
        start.elapsed().as_secs_f64()
    );

    if debug && !errors.is_empty() {
        dump(&errors, "lastrun.log").unwrap_or_else(|_| eprintln!("Failed to dump errors."));
        println!("Dumped all errors to a log file.");
    }

    // any errors stop at the lexing stage
    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}