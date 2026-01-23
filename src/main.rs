//! docs here soon i'm lazy as fuck
mod lexer;
mod parser;

// gotta work on this name but now im tired
use crate::{
    lexer::{diagnostic::dump, lex::lex, token::Token},
    parser::{ast::Stmt, parse::Parser},
};

use std::{
    env::{Args, args},
    fs::read_to_string,
    iter::Skip,
    process::exit,
};

fn main() {
    // handle cl args
    let mut args: Skip<Args> = args().skip(1);
    let mut path: Option<String> = None;

    // flags live in a vector (0 = debug, 1 = fast fail. maybe hashmap but thats dumb cemantics i wanna setup the parser)
    let mut flags: Vec<bool> = vec![false; 2];
    while let Some(a) = args.next() {
        match a.as_str() {
            "-d" | "--debug" => flags[0] = true,
            "-ff" | "--fastfail" => flags[1] = true,
            "--" => {
                if let Some(p) = args.next() {
                    path = Some(p);
                }
                break;
            }

            // unknown args
            s if s.starts_with('-') => usage!("unknown flag: {}\n", s),
            s => {
                if path.is_none() {
                    path = Some(s.to_string())
                }
            }
        }
    }

    // try and open properly
    let path: String = path.unwrap_or_else(|| {
        usage!();
    });
    let src: String = read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("failed to read {path}: {e}");
        exit(0);
    });

    // TODO: parse the returned tokens into an AST
    let tokens: Vec<Token<'_>> = match lex(&path, &src, flags[0], flags[1]) {
        Ok(tokens) => tokens,

        // any errors
        Err(errors) => {
            for d in &errors {
                eprintln!("{d}");
            }

            // debug flag
            if flags[0] {
                dump(&errors, "lastrun.log")
                    .unwrap_or_else(|_| eprintln!("Failed to dump errors."));
            }

            println!("\n(!) {} errors found.", errors.len());
            exit(0);
        }
    };

    if flags[0] { press_btn_continue::wait("Press any button to continue to parsing.").unwrap(); }
    let mut parser = Parser {
        path: &path,
        src: &src,
        tokens: &tokens,
        pos: 0,
    };

    let _ast: Vec<Stmt<'_>> = match parser.parse(flags[0]) {
        Ok(ok) => ok,
        Err(_err) => exit(0),
    };

    if flags[0] { press_btn_continue::wait("Press any button to continue to semantic analysis and the opt layer. (not done yet)").unwrap(); }
}
