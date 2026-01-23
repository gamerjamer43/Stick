use core::fmt;
use std::fmt::Display;

// the lexer itself, the big beef (logos specs look a lil ugly so don't count this in any PRs)
use super::error::{SyntaxError, err};
use logos::{Logos, skip};

// the entire token spec. this also doubles as the lexer itself when we run Token::lexer()
#[derive(Logos, Default, Debug, PartialEq)]
#[logos(error(SyntaxError<'s>, err))] // TODO: fully understand why 's shuts this up, and why i can't use '_ or 'src
#[logos(skip r"[ \t\n\f\r]+")] // ignore tabs, newlines, form feeds, and carriage returns
pub enum Token<'src> {
    // comments (skipped)
    #[regex(r"//[^\r\n]*", skip, allow_greedy = true)]
    #[regex(r"/\*([^*]|\*+[^*/])*\*+/", skip)]
    Comment,

    // equality and comparisons
    #[token("==")]  EqEq,
    #[token("!=")]  NotEq,
    #[token("<=")]  LessEq,
    #[token(">=")]  GreaterEq,

    // shifts and compound assigns
    #[token("<<=")] ShlEq,
    #[token(">>=")] ShrEq,
    #[token("<<")]  Shl,
    #[token(">>")]  Shr,

    // arithmetic compound assigns
    #[token("+=")]  PlusEq,
    #[token("-=")]  MinusEq,
    #[token("*=")]  StarEq,
    #[token("/=")]  SlashEq,
    #[token("%=")]  PercentEq,

    // bitwise compound assigns
    #[token("&=")]  AndEq,
    #[token("|=")]  OrEq,
    #[token("^=")]  XorEq,

    // punctuation and others (member access and shit)
    #[token("::")]  DoubleColon,
    #[token("->")]  Arrow,
    #[token("=>")]  FatArrow,
    #[token("|->")] Branch,

    // ranges and varargs style (may not use)
    #[token("...")] Elipses,
    #[token("..")]  DotDot,

    // arithmetic
    #[token("**")]  StarStar,
    #[token("+")]   Plus,
    #[token("-")]   Minus,
    #[token("*")]   Star,
    #[token("/")]   Slash,
    #[token("%")]   Percent,

    // assignment
    #[token("=")]   Assign,

    // comparisons
    #[token("<")]   Less,
    #[token(">")]   Greater,

    // bitwise
    #[token("&")]   BitAnd,
    #[token("|")]   BitOr,
    #[token("^")]   BitXor,
    #[token("~")]   BitNot,

    // logical ops
    #[token("not")] LogicalNot,
    #[token("and")] LogicalAnd,
    #[token("or")]  LogicalOr,

    // punctuation
    #[token(".")]   Dot,
    #[token(":")]   Colon,
    #[token("?")]   Question,

    // grouping / separators
    #[token("(")]   LParen,
    #[token(")")]   RParen,
    #[token("[")]   LBracket,
    #[token("]")]   RBracket,
    #[token("{")]   LBrace,
    #[token("}")]   RBrace,
    #[token(",")]   Comma,
    #[token(";")]   Semicolon,

    /// true or false is properly handled at parse time as a boolean
    #[token("false", |_| false)]
    #[token("true", |_| true)]
    Bool(bool),

    // fine ig i'll do control keywords in here too
    #[token("if")]       If,
    #[token("else")]     Else,
    #[token("fn")]       Fn,
    #[token("while")]    While,
    #[token("do")]       Do,
    #[token("for")]      For,
    #[token("in")]       In,
    #[token("return")]   Return,
    #[token("break")]    Break,
    #[token("continue")] Continue,
    #[token("match")]    Match,
    #[token("import")]   Import,
    #[token("from")]     From,


    // type qualifiers/storage specifiers
    #[token("const")]    Const,
    #[token("static")]   Static,
    #[token("public")]   Public,

    // object oriented and imperative shit
    #[token("class")]    Class,
    #[token("struct")]   Struct,
    #[token("enum")]     Enum,

    // lifetimes like 'a (may add to give the user more handlage, trying to allow for any paradigm the user wants)
    // #[regex(r"'[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice())]
    // Lifetime(&'src str),

    /// identifiers cannot start with a number, and can only contain A-Z, a-z, 0-9, and _
    #[regex("[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice())]
    Identifier(&'src str),

    /// strings are anything enclosed inside "", no closure == borrow slices of the string at parse time, avoids heap allocating a copy
    // #[regex(r#""([^"\\\n]|\\.)*""#, |lex| lex.slice().to_owned())]
    #[regex(r#""([^"\\\n]|\\.)*""#, |lex| lex.slice())]
    LitString(&'src str),

    /// chars are single characters (including escaped chars) enclosed in ''
    #[regex(r#"'([^'\\\n]|\\.)'"#, |lex| lex.slice())]
    LitChar(&'src str),

    #[regex(r"[0-9]+(?:_[0-9]+)*\.[0-9]+", |lex| lex.slice())]
    LitFloat(&'src str),

    #[regex(r"[0-9]+(?:_[0-9]+)*", |lex| lex.slice())]
    LitInteger(&'src str),

    // this will throw a SyntaxError
    #[default]
    Error,
}

// everything im not too lazy to add a display for. gotta add some more obv
impl<'src> Display for Token<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::EqEq        => f.write_str("=="),
            Token::NotEq       => f.write_str("!="),
            Token::LessEq      => f.write_str("<="),
            Token::GreaterEq   => f.write_str(">="),

            Token::Plus        => f.write_str("+"),
            Token::Minus       => f.write_str("-"),
            Token::Star        => f.write_str("*"),
            Token::Slash       => f.write_str("/"),

            Token::Assign      => f.write_str("="),

            Token::If          => f.write_str("if"),
            Token::Else        => f.write_str("else"),
            Token::Fn          => f.write_str("fn"),

            Token::Bool(v)     => write!(f, "{v}"),
            Token::Identifier(s)
            | Token::LitString(s)
            | Token::LitChar(s)
            | Token::LitFloat(s)
            | Token::LitInteger(s) => f.write_str(s),

            Token::Error       => f.write_str("<error>"),

            // fallback for everything else
            _ => write!(f, "{:?}", self),
        }
    }
}