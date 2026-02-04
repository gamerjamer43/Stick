#![allow(dead_code, unused_variables)]

use core::fmt;
use std::{mem::take, ops::Range, process::exit, time::Instant};

use super::ast::*;
use crate::error::{Diagnostic, ParseError, SyntaxError};
use crate::lexer::Token;

// didn't tie parser lifetime to source
pub struct Parser<'src, 't> {
    pub path: &'src str,
    pub src: &'src str,
    pub tokens: &'t [Token<'src>],
    pub spans: &'t [Range<usize>],
    pub pos: usize,
    pub fastfail: bool,
    pub errors: Vec<Diagnostic<'t, 'src>>,
}

impl<'src, 't> Parser<'src, 't> {
    #[inline]
    fn cur(&self) -> Option<&'t Token<'src>> {
        self.tokens.get(self.pos)
    }

    #[inline]
    fn peek(&self) -> Option<&'t Token<'src>> {
        self.tokens.get(self.pos + 1)
    }

    #[inline]
    fn matches(&self, matched: &Token<'src>) -> bool {
        let tok: &Token<'_> = self.cur().unwrap_or(&Token::Error);
        tok == matched
    }

    #[inline]
    // i'm so smart. match any using a slice to reduce the amt of calls to self.matches
    fn matches_any(&self, any: &[Token<'src>]) -> bool {
        let tok: &Token<'_> = self.cur().unwrap_or(&Token::Error);
        any.contains(tok)
    }

    #[inline]
    fn error(&mut self, err: SyntaxError<'src>) {
        let diag: Diagnostic<'_, '_> = Diagnostic {
            path: self.path,
            src: self.src,

            // small copy whatever
            span: self.spans[self.pos].clone(),
            err,
        };

        if self.fastfail {
            println!("{diag}");
            exit(0);
        }

        self.errors.push(diag);
    }

    #[inline]
    fn expect<F>(&mut self, f: F) -> Option<&Token<'src>>
    where
        F: FnOnce(&Token<'_>) -> bool,
    {
        let tok = self.cur()?;
        if f(tok) {
            self.pos += 1;
            Some(tok)
        } else {
            // TODO: add proper error handling
            None
        }
    }

    // leaving some shit intentionally _ because i'll do lifetimes later
    #[inline]
    fn advance(&mut self) -> Option<&Token<'src>> {
        self.advance_by(1)
    }

    #[inline]
    fn advance_by(&mut self, n: u8) -> Option<&Token<'src>> {
        let tok: &Token<'src> = self.cur()?;
        self.pos += n as usize;
        Some(tok)
    }

    // TODO: add plain ranges. val = 1..3
    // TODO 2: add dest and type based decls. decide i64 int = 1 or let int: i64 = 1 and const, global, maybe static too: const i64 int = 1 or let const int: i64 = 1
    // TODO 3: make semicolons OPTIONAL at the end of a line (or to end a statement)
    #[inline]
    fn parse_expr(&mut self, min: u8) -> Expr<'src> {
        // check for anything before
        let mut left: Expr<'_> = self.parse_prefix();

        // get the token into scope
        while let Some(tok) = self.cur() {
            let tok: &Token<'_> = match self.cur() {
                Some(tok) => tok,
                None => {
                    println!("not implemented: {tok:?}");
                    return Expr::Unknown;
                }
            };

            // indexing/fields r highest precedence
            let precedence: u8 = match tok {
                Token::LParen
                | Token::LBracket
                | Token::Dot
                | Token::Arrow
                | Token::PlusPlus
                | Token::MinusMinus => 15,

                _ => 0,
            };

            // oh this nesting makes me keel
            if precedence != 0 && precedence >= min {
                match tok {
                    // function calls
                    Token::LParen => {
                        self.advance();

                        // eat as many args as possible. default to take 8 before resizing then its ur problem lmao
                        let mut args: Vec<Expr<'_>> = Vec::with_capacity(8);
                        if !self.matches(&Token::RParen) {
                            args.push(self.parse_expr(0));

                            // match commas (and ending parenthesis)
                            while self.matches(&Token::Comma) {
                                self.advance();
                                if self.matches(&Token::RParen) {
                                    break;
                                }

                                // evaluate THEN push
                                args.push(self.parse_expr(0));
                            }

                            // malformed calls
                            if !self.matches(&Token::RParen) {
                                self.error(
                                    SyntaxError::Parse(ParseError::MissingExpected("expected ',' or ')' in call. have to add this to the error system"))
                                );
                            }
                        }

                        // expect r paren
                        self.expect(|t: &Token<'_>| matches!(t, Token::RParen))
                            .expect("missing ')'");

                        // method calls exist, so there's a match here
                        left = match left {
                            Expr::Field { obj, name } => Expr::Method {
                                receiver: obj,
                                method: name,
                                args,
                            },

                            // also boxing to avoid infinite recursive eval
                            other => Expr::Call {
                                func: Box::new(other),
                                args,
                            },
                        };
                    }

                    // TODO: discriminate dot vs arrow
                    Token::Dot | Token::Arrow => {
                        self.advance();

                        // fields r simple just should be one identifier
                        let name = match self.advance() {
                            Some(Token::Identifier(name)) => name,
                            _ => {
                                println!("not implemented: {tok:?}");
                                return Expr::Unknown;
                            }
                        };

                        let lvalue: Box<Expr<'_>> = Box::new(left);
                        left = Expr::Field {
                            obj: lvalue,
                            name: Ident(name),
                        };
                    }

                    // slices/index
                    Token::LBracket => {
                        self.advance();

                        // slices are denoted [start..end], [start..] or [..end]
                        let sub: Subscript<'_> = if self.matches(&Token::DotDot) {
                            self.advance();

                            // match the end bracket or error
                            let end: Option<Box<Expr<'_>>> = if !self.matches(&Token::RBracket) {
                                Some(Box::new(self.parse_expr(0)))
                            } else {
                                None
                            };

                            Subscript::Range { start: None, end }
                        } else {
                            // otherwise try and evaluate out whatever is inside, start then end
                            let start: Expr<'_> = self.parse_expr(0);
                            if self.matches(&Token::DotDot) {
                                self.advance();

                                // if nothing matches its [i..]
                                let end: Option<Box<Expr<'_>>> = if !self.matches(&Token::RBracket)
                                {
                                    Some(Box::new(self.parse_expr(0)))
                                } else {
                                    None
                                };

                                Subscript::Range {
                                    start: Some(Box::new(start)),
                                    end,
                                }
                            }
                            // NOW we know it's an index
                            else {
                                Subscript::Index(Box::new(start))
                            }
                        };

                        // expect an ending bracket
                        self.expect(|t: &Token<'_>| matches!(t, Token::RBracket))
                            .expect("missing ']'");

                        let lvalue: Box<Expr<'_>> = Box::new(left);
                        left = Expr::Index { obj: lvalue, sub };
                    }

                    // postfix increment/decrement
                    Token::PlusPlus => {
                        self.advance();
                        left = Expr::Unary {
                            op: UnaryOp::PostInc,
                            expr: Box::new(left),
                        };
                    }

                    Token::MinusMinus => {
                        self.advance();
                        left = Expr::Unary {
                            op: UnaryOp::PostDec,
                            expr: Box::new(left),
                        };
                    }

                    // never hits if this hits ur dumb
                    _ => unreachable!("how. this is in parse expr as part of the indexing/slicing"),
                }

                continue;
            }

            // normal ops
            let (op_prec, op) = match tok {
                // assignment always last trump
                Token::PlusEq => (0, InfixKind::Assign(AssignOp::PlusEq)),
                Token::MinusEq => (0, InfixKind::Assign(AssignOp::MinusEq)),
                Token::StarEq => (0, InfixKind::Assign(AssignOp::StarEq)),
                Token::SlashEq => (0, InfixKind::Assign(AssignOp::SlashEq)),
                Token::PercentEq => (0, InfixKind::Assign(AssignOp::PercentEq)),
                Token::AndEq => (0, InfixKind::Assign(AssignOp::AndEq)),
                Token::OrEq => (0, InfixKind::Assign(AssignOp::OrEq)),
                Token::XorEq => (0, InfixKind::Assign(AssignOp::XorEq)),
                Token::ShlEq => (0, InfixKind::Assign(AssignOp::ShlEq)),
                Token::ShrEq => (0, InfixKind::Assign(AssignOp::ShrEq)),

                // logical/bitwise
                Token::LogicalOr => (1, InfixKind::Binary(BinOp::Or)),
                Token::LogicalAnd => (2, InfixKind::Binary(BinOp::And)),
                Token::BitOr => (3, InfixKind::Binary(BinOp::BitOr)),
                Token::BitXor => (4, InfixKind::Binary(BinOp::BitXor)),
                Token::BitAnd => (5, InfixKind::Binary(BinOp::BitAnd)),
                Token::EqEq => (6, InfixKind::Binary(BinOp::Eq)),
                Token::NotEq => (6, InfixKind::Binary(BinOp::NotEq)),

                // comparators
                Token::Less | Token::LessEq | Token::Greater | Token::GreaterEq => match tok {
                    Token::Less => (7, InfixKind::Binary(BinOp::Less)),
                    Token::LessEq => (7, InfixKind::Binary(BinOp::LessEq)),
                    Token::Greater => (7, InfixKind::Binary(BinOp::Greater)),
                    Token::GreaterEq => (7, InfixKind::Binary(BinOp::GreaterEq)),
                    _ => unreachable!("what"),
                },

                // then comes assign its first match
                Token::Assign => (0, InfixKind::Assign(AssignOp::Assign)),

                // bit shifts
                Token::Shl | Token::Shr => match tok {
                    Token::Shl => (8, InfixKind::Binary(BinOp::Shl)),
                    Token::Shr => (8, InfixKind::Binary(BinOp::Shr)),
                    _ => unreachable!("huh"),
                },

                // AS
                Token::Plus | Token::Minus => match tok {
                    Token::Plus => (9, InfixKind::Binary(BinOp::Add)),
                    Token::Minus => (9, InfixKind::Binary(BinOp::Sub)),
                    _ => unreachable!("what the helly"),
                },

                // MD (m = mult AND modulo)
                Token::Star | Token::Slash | Token::Percent => match tok {
                    Token::Star => (10, InfixKind::Binary(BinOp::Mul)),
                    Token::Slash => (10, InfixKind::Binary(BinOp::Div)),
                    Token::Percent => (10, InfixKind::Binary(BinOp::Mod)),
                    _ => unreachable!("what the helliante"),
                },

                // E
                Token::StarStar => (11, InfixKind::Binary(BinOp::Power)),

                // range operator is always lower than normal arithmetic
                Token::DotDot => {
                    self.advance();
                    
                    // parse end of range if present (not followed by { or delimiter)
                    let end: Option<Box<Expr<'_>>> = if !self.matches_any(&[Token::LBrace, Token::Newline, Token::Semicolon, Token::RBracket, Token::RParen]) {
                        Some(Box::new(self.parse_expr(0)))
                    } 
                    
                    else { None };

                    left = Expr::Range {
                        start: Some(Box::new(left)),
                        end,
                    };
                    continue;
                }

                // erm
                _ => break,
            };

            // let higher precedence ops finish first
            if op_prec < min {
                break;
            }
            self.advance();

            match op {
                InfixKind::Assign(aop) => {
                    // assignments come last. otherwise left assoc
                    let rhs: Expr<'_> = self.parse_expr(op_prec);

                    let lhs = match left {
                        Expr::Ident(ident) => LeftSide::Var(ident),
                        Expr::Field { obj, name } => LeftSide::Field { obj, name },
                        Expr::Index { obj, sub } => LeftSide::Subscript { obj, sub },
                        _ => {
                            println!("not implemented: or something went wrong {tok:?}");
                            return Expr::Unknown;
                        }
                    };

                    left = Expr::Assign {
                        op: aop,
                        lhs,
                        rhs: Box::new(rhs),
                    };
                }

                InfixKind::Binary(bop) => {
                    let rhs: Expr<'_> = self.parse_expr(op_prec + 1);
                    left = Expr::Binary {
                        op: bop,
                        lhs: Box::new(left),
                        rhs: Box::new(rhs),
                    };
                }
            }
        }

        left
    }

    // gonna change the name, but this is all patterns that can be in a for loop (and potentially match and shit)
    fn parse_pattern(&mut self) -> Pattern<'src> {
        match self.cur() {
            Some(Token::Underscore) => {
                self.advance();
                Pattern::Wildcard
            }

            Some(Token::DotDot) => {
                self.advance();

                // parse end if present
                let end: Option<Box<Expr<'_>>> = if !self.matches_any(&[Token::LBrace, Token::Newline, Token::Semicolon]) {
                    Some(Box::new(self.parse_expr(0)))
                } 
                
                else {
                    None
                };

                Pattern::Range { start: None, end }
            }

            Some(Token::Identifier(name)) => {
                let n = *name;
                self.advance();
                Pattern::Ident(n)
            }

            // tuples
            Some(Token::LParen) => {
                self.advance();
                let mut patterns = vec![self.parse_pattern()];

                while self.matches(&Token::Comma) {
                    self.advance();
                    patterns.push(self.parse_pattern());
                }

                self.expect(|t| matches!(t, Token::RParen))
                    .expect("expected ')' in tuple pattern");
                
                Pattern::Tuple(patterns)
            }

            // default
            _ => Pattern::Wildcard,
        }
    }

    fn parse_for_expr(&mut self) -> Expr<'src> {
        // TODO replace the many expect calls with self.error this is the only way it was workin
        let pattern: Pattern<'_> = self.parse_pattern();

        // syntax: for _ in r1..r2 (TODO figure out step amts)
        self.expect(|t: &Token<'_>| matches!(t, Token::In))
            .expect("missing keyword 'in' inside for loop");

        let iter = self.parse_expr(0);

        // for loops need braces for right now (TODO one line for loops)
        self.expect(|t: &Token<'_>| matches!(t, Token::LBrace))
            .expect("expected '{' in for loop");
        
        let body: Expr<'_> = self.parse_block_expr();
        
        Expr::For {
            pattern,
            iter: Box::new(iter),
            body: Box::new(body),
        }
    }

    fn parse_if_expr(&mut self) -> Expr<'src> {
        let cond: Expr<'_> = self.parse_expr(0);
        self.expect(|t: &Token<'_>| matches!(t, Token::LBrace))
            .expect("missing '{' before if body");

        let then: Expr<'_> = self.parse_block_expr();

        // ONLY eat newlines when there's an else clause, otherwise the parser needs it as a delimiter
        let checkpoint: usize = self.pos;
        while self.matches(&Token::Newline) {
            self.advance();
        }

        // check for an else statement, and skip present newlines
        let else_: Option<Box<Expr<'_>>> = if self.matches(&Token::Else) {
            self.advance();
            while self.matches(&Token::Newline) {
                self.advance();
            }

            // recursively parse as an Else { If {} } we have an else if
            if self.matches(&Token::If) {
                self.advance();
                Some(Box::new(self.parse_if_expr()))
            }
            // otherwise make sure that brace is there (and then parse)
            else {
                self.expect(|t: &Token<'_>| matches!(t, Token::LBrace))
                    .expect("missing '{' before else body");
                Some(Box::new(self.parse_block_expr()))
            }
        }
        // otherwise no else
        else {
            self.pos = checkpoint;
            None
        };

        Expr::If {
            cond: Box::new(cond),
            then: Box::new(then),
            else_,
        }
    }

    // TODO replace any .expect("msg") calls with self.error. this is the only way it was workin frn
    fn parse_while_expr(&mut self) -> Expr<'src> {
        let cond: Expr<'_> = self.parse_expr(0);
        self.expect(|t: &Token<'_>| matches!(t, Token::LBrace))
            .expect("missing '{' before while body");

        let body: Expr<'_> = self.parse_block_expr();

        Expr::While {
            cond: Box::new(cond),
            body: Box::new(body),
        }
    }

    fn parse_match_expr(&mut self) -> Expr<'src> {
        todo!("gabagool")
    }

    fn parse_block_expr(&mut self) -> Expr<'src> {
        let mut stmts: Vec<Stmt<'src>> = Vec::new();
        let mut tail: Option<Box<Expr<'src>>> = None;

        loop {
            let tok = match self.cur() {
                Some(t) => t,

                // no closing } errors out
                None => {
                    self.error(SyntaxError::Parse(ParseError::MissingExpected(
                        "expected '}' to close block",
                    )));
                    break;
                }
            };

            match tok {
                // break on right brace
                Token::RBrace => {
                    self.advance();
                    break;
                }

                // pass newlines
                Token::Newline => {
                    self.advance();
                    continue;
                }

                Token::Let => match self.parse_let() {
                    Ok(stmt) => stmts.push(stmt),
                    Err(e) => self.error(e),
                },

                Token::Return => {
                    self.advance();

                    // return NONE
                    if self.matches_any(&[Token::Newline, Token::Semicolon, Token::RBrace]) {
                        stmts.push(Stmt::Return(None));
                    }
                    // return an expression
                    else {
                        let expr = self.parse_expr(0);
                        stmts.push(Stmt::Return(Some(expr)));
                    }
                }

                Token::Break => {
                    self.advance();
                    stmts.push(Stmt::Break);
                }

                Token::Continue => {
                    self.advance();
                    stmts.push(Stmt::Continue);
                }

                // all statements matched, expect an expression
                _ => {
                    let expr = self.parse_expr(0);

                    // skip newlines (this was fuckin up tails)
                    while self.matches(&Token::Newline) {
                        self.advance();
                    }

                    if self.matches(&Token::RBrace) {
                        tail = Some(Box::new(expr));

                        self.advance();
                        break;
                    }

                    stmts.push(Stmt::Expr(expr));
                    continue;
                }
            }

            // delimiter handling inside block
            if self.matches_any(&[Token::Newline, Token::Semicolon]) {
                while self.matches_any(&[Token::Newline, Token::Semicolon]) {
                    self.advance();
                }
                continue;
            }

            if self.matches(&Token::RBrace) {
                self.advance();
                break;
            }

            self.error(SyntaxError::Parse(ParseError::MissingExpected(
                "expected ';', newline, or '}' after statement in block",
            )));
        }

        Expr::Block { stmts, tail }
    }

    #[inline]
    fn parse_prefix(&mut self) -> Expr<'src> {
        // TODO: write proper error handling... and parse expr... and test this
        let tok: &Token<'_> = self.advance().expect("unexpected EOF");
        match tok {
            Token::Minus => Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(self.parse_expr(12)),
            },
            Token::MinusMinus => Expr::Unary {
                op: UnaryOp::PreDec,
                expr: Box::new(self.parse_expr(12)),
            },
            Token::PlusPlus => Expr::Unary {
                op: UnaryOp::PreInc,
                expr: Box::new(self.parse_expr(12)),
            },
            Token::LogicalNot => Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_expr(12)),
            },
            Token::BitNot => Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(self.parse_expr(12)),
            },

            // prefix ranges (for slicing)
            Token::DotDot => {
                let end: Option<Box<Expr<'_>>> = if !self.matches_any(&[Token::LBrace, Token::Newline, Token::Semicolon, Token::RBracket, Token::RParen]) {
                    Some(Box::new(self.parse_expr(0)))
                } 
                
                else {
                    None
                };

                Expr::Range { start: None, end }
            }

            Token::LParen => {
                let inner = self.parse_expr(0);
                self.expect(|t: &Token<'_>| matches!(t, Token::RParen))
                    .expect("missing ')'");
                inner
            }

            Token::Identifier(name) => Expr::Ident(Ident(name)),
            Token::LitInteger(n) => Expr::Literal(Literal::Int(n)),
            Token::LitFloat(n) => Expr::Literal(Literal::Float(n)),
            Token::LitString(s) => Expr::Literal(Literal::String(s)),
            Token::LitChar(c) => Expr::Literal(Literal::Char(c)),
            Token::Bool(b) => Expr::Literal(Literal::Bool(*b)),

            Token::If => self.parse_if_expr(),
            Token::While => self.parse_while_expr(),
            Token::For => self.parse_for_expr(),
            Token::Match => self.parse_match_expr(),

            Token::LBrace => self.parse_block_expr(),

            // temporary solution for nimpl, i need to link ariadne
            _ => {
                // add this back w a debug flag idk if debug { println!("not implemented: {tok:?}"); }
                Expr::Unknown
            }
        }
    }

    pub fn parse_let(&mut self) -> Result<Stmt<'src>, SyntaxError<'src>> {
        self.advance();

        // specifiers are evaluated in this order
        let constant = self.expect(|t| matches!(t, Token::Const)).is_some();
        let global = self.expect(|t| matches!(t, Token::Static)).is_some();
        let mutable = self.expect(|t| matches!(t, Token::Mutable)).is_some();

        // ensure constant isnt used where it can't be
        if constant && mutable {
            self.error(SyntaxError::Parse(ParseError::ConstDisallowed(
                "constant cannot be used in tandem with mutable.",
            )));
        }
        if constant && global {
            self.error(SyntaxError::Parse(ParseError::ConstDisallowed(
                "constant cannot be used in tandem with static.",
            )));
        }

        // consume name (TODO: add let _)
        let name: Ident<'_> = match self.expect(|t| matches!(t, Token::Identifier(_))) {
            Some(Token::Identifier(name)) => Ident(name),
            _ => {
                return Err(SyntaxError::Parse(ParseError::MissingExpected(
                    "let must have an identifier afterwards",
                )));

                // continue;
            }
        };

        // consume annotation
        let typ: Type<'_> = if self.matches(&Token::Colon) {
            self.advance();

            // TODO: add support for array and generic types
            match self
                .expect(|t| matches!(t, Token::Identifier(_) | Token::Unit | Token::Underscore))
            {
                Some(Token::Identifier(typname)) => match *typname {
                    "i8" => Type::I8,
                    "u8" => Type::U8,
                    "i16" => Type::I16,
                    "u16" => Type::U16,
                    "i32" => Type::I32,
                    "u32" => Type::U32,
                    "i64" => Type::I64,
                    "u64" => Type::U64,
                    "f32" => Type::F32,
                    "f64" => Type::F64,
                    "bool" => Type::Bool,
                    "char" => Type::Char,
                    "str" => Type::Str,
                    _ => Type::Ident(Ident(typname)),
                },

                // unit type and inferred have to be handled seperately
                Some(Token::Unit) => Type::Unit,
                Some(Token::Underscore) => Type::Inferred,

                // push missing type after :
                _ => {
                    return Err(SyntaxError::Parse(ParseError::MissingExpected(
                        "expected type name after ':'",
                    )));
                }
            }
        }
        // (if no annotation the type is inferred by the compiler)
        else {
            Type::Inferred
        };

        // expected equals to get to right hand of assignment (if none it's a decl)
        let mut init = None;
        if self.matches(&Token::Assign) {
            self.advance();

            match self.cur().unwrap_or(&Token::Error) {
                Token::Error | Token::Newline | Token::Semicolon | Token::Eof => {
                    return Err(SyntaxError::Parse(ParseError::MissingExpected(
                        "expected expression after '='",
                    )));
                }

                _ => {
                    init = Some(self.parse_expr(0));
                }
            }
        }

        // can't automatically deduce type on assignment (maybe make it so that the type is filled when assigned to?)
        if typ == Type::Inferred && init.is_none() {
            return Err(SyntaxError::Parse(ParseError::MissingExpected(
                "type cannot be inferred without a right hand side",
            )));
        }

        // we did it!!!!
        Ok(Stmt::VarDecl {
            name,
            typ,
            init,
            mutable,
            constant,
            global,
        })
    }

    pub fn parse(&mut self, flags: &[bool]) -> Result<Vec<Stmt<'src>>, Vec<Diagnostic<'t, 'src>>> {
        let mut nodes: Vec<Stmt<'src>> = Vec::new();
        let start: Instant = Instant::now();

        // resolve flags
        let debug: bool = flags[0];
        let fastfail: bool = flags[1];
        self.fastfail = fastfail;
        if debug {
            println!();
        }

        while let Some(cur) = self.cur() {
            match cur {
                // skip newlines (and eof its just handled by checking none)
                Token::Newline | Token::Eof => {
                    self.advance();
                    continue;
                }

                // expression statements (idents, literals, blocks, if, and others)
                Token::Identifier(_)
                | Token::LitInteger(_)
                | Token::LitFloat(_)
                | Token::LitString(_)
                | Token::LitChar(_)
                | Token::Bool(_)
                | Token::LParen
                | Token::LBrace
                | Token::If
                | Token::While
                | Token::For
                | Token::Minus
                | Token::PlusPlus
                | Token::MinusMinus
                | Token::LogicalNot
                | Token::BitNot => nodes.push(Stmt::Expr(self.parse_expr(0))),

                // TODO: see how we can break some of this down
                Token::Let => match self.parse_let() {
                    Ok(stmt) => nodes.push(stmt),
                    Err(e) => self.error(e),
                },

                // control flow: this dont seem right but...
                // Token::Break => nodes.push(Stmt::Break),
                // Token::Continue => nodes.push(Stmt::Continue),

                // TODO: wire this to the SyntaxError setup i alr have
                _ => {
                    if debug {
                        println!("not implemented: {cur}");
                    }
                    self.advance();
                    continue;
                }
            }

            if debug {
                println!("Parsed: \n{:#?}\n", nodes.last().unwrap());
            }

            // TODO: make the compiler warn on unnecessary semicolon
            if !(self.matches_any(&[Token::Newline, Token::Semicolon, Token::Eof, Token::LBrace])) {
                self.error(SyntaxError::Parse(ParseError::MissingExpected(
                    "all statements must be followed by either a newline or semicolon",
                )));
                continue;
            }

            while self.matches(&Token::Semicolon) {
                self.advance();
            }
        }

        println!(
            "Parsed {} tokens into {} nodes. Took {}s.",
            self.tokens.len(),
            nodes.len(),
            start.elapsed().as_secs_f64()
        );

        if self.errors.is_empty() {
            Ok(nodes)
        } else {
            // i prolly dont have to do a move here... but wtv for rn
            Err(take(&mut self.errors))
        }
    }
}

impl<'src, 't> fmt::Display for Parser<'src, 't> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!("have to do the display")
    }
}
