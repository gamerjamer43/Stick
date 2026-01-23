#![allow(dead_code, unused_variables)]

use core::fmt;
use std::time::Instant;

use super::ast::*;
use crate::lexer::{diagnostic::Diagnostic, token::Token};

// didn't tie parser lifetime to source
pub struct Parser<'src, 't> {
    pub path: &'src str,
    pub src: &'src str,
    pub tokens: &'t [Token<'src>],
    pub pos: usize,
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
    fn expect<F>(&mut self, f: F) -> Option<&Token<'src>>
    where
        F: FnOnce(&Token<'_>) -> bool,
    {
        let tok = self.cur()?;
        if f(tok) {
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    // leaving some shit intentionally _ because i'll do lifetimes later
    #[inline] fn advance(&mut self) -> Option<&Token<'src>> { self.advance_by(1) }

    #[inline]
    fn advance_by(&mut self, n: u8) -> Option<&Token<'src>> {
        let tok: &Token<'src> = self.cur()?;
        self.pos += n as usize;
        Some(tok)
    }

    // TODO: add plain ranges. val = 1..3
    // TODO 2: also add typed decls. i64 int = 1;
    // TODO 3: figure out how to use semicolons. prolly for one line statements use it as a seperator, but no semicolon required so
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
                },
            };

            // indexing/fields r highest precedence
            let precedence: u8 = match tok {
                Token::LParen | Token::LBracket | Token::Dot => 15,
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
                        if !matches!(self.cur(), Some(Token::RParen)) {
                            args.push(self.parse_expr(0));

                            // match commas (and ending parenthesis)
                            while matches!(self.cur(), Some(Token::Comma)) {
                                self.advance();
                                if matches!(self.cur(), Some(Token::RParen)) { break; }

                                // evaluate THEN push
                                args.push(self.parse_expr(0));
                            }

                            // malformed calls
                            if !matches!(self.cur(), Some(Token::RParen)) { panic!("expected ',' or ')' in call. still have yet to add an error system"); }
                        }

                        // expect r paren
                        self.expect(|t: &Token<'_>| matches!(t, Token::RParen)).expect("missing ')'");

                        // also boxing to avoid infinite recursive eval
                        let lvalue: Box<Expr<'_>> = Box::new(left);
                        left = Expr::Call { func: lvalue, args };
                    }

                    // TODO: add method calls (maybe just wrap call around a field?)
                    Token::Dot => {
                        self.advance();

                        // fields r simple just should be one identifier
                        let name = match self.advance() {
                            Some(Token::Identifier(name)) => name,
                            _ => {
                                println!("not implemented: {tok:?}");
                                return Expr::Unknown;
                            },
                        };

                        let lvalue: Box<Expr<'_>> = Box::new(left);
                        left = Expr::Field { obj: lvalue, name };
                    }

                    // slices/index
                    Token::LBracket => {
                        self.advance();

                        // slices are denoted [start..end], [start..] or [..end]
                        let sub: Subscript<'_> = if matches!(self.cur(), Some(Token::DotDot)) {
                            self.advance();

                            // match the end bracket or error
                            let end: Option<Box<Expr<'_>>> = if !matches!(self.cur(), Some(Token::RBracket)) {
                                Some(Box::new(self.parse_expr(0)))
                            } else { None };

                            Subscript::Range { start: None, end }
                        }

                        else {
                            // otherwise try and evaluate out whatever is inside, start then end
                            let start: Expr<'_> = self.parse_expr(0);
                            if matches!(self.cur(), Some(Token::DotDot)) {
                                self.advance();

                                // if nothing matches its [i..]
                                let end: Option<Box<Expr<'_>>> = if !matches!(self.cur(), Some(Token::RBracket)) {
                                    Some(Box::new(self.parse_expr(0)))
                                } else { None };

                                Subscript::Range { start: Some(Box::new(start)), end }
                            }

                            // NOW we know it's an index
                            else { Subscript::Index(Box::new(start)) }
                        };

                        // expect an ending bracket
                        self.expect(|t: &Token<'_>| matches!(t, Token::RBracket)).expect("missing ']'");

                        let lvalue: Box<Expr<'_>> = Box::new(left);
                        left = Expr::Index { obj: lvalue, sub };
                    }

                    // never hits if this hits ur dumb
                    _ => unreachable!("how. this is in parse expr as part of the indexing/slicing"),
                }

                continue;
            }

            // normal ops
            let (op_prec, op) = match tok {
                // assignment always last trump
                Token::PlusEq    => (0, InfixKind::Assign(AssignOp::PlusEq)),
                Token::MinusEq   => (0, InfixKind::Assign(AssignOp::MinusEq)),
                Token::StarEq    => (0, InfixKind::Assign(AssignOp::StarEq)),
                Token::SlashEq   => (0, InfixKind::Assign(AssignOp::SlashEq)),
                Token::PercentEq => (0, InfixKind::Assign(AssignOp::PercentEq)),
                Token::AndEq     => (0, InfixKind::Assign(AssignOp::AndEq)),
                Token::OrEq      => (0, InfixKind::Assign(AssignOp::OrEq)),
                Token::XorEq     => (0, InfixKind::Assign(AssignOp::XorEq)),
                Token::ShlEq     => (0, InfixKind::Assign(AssignOp::ShlEq)),
                Token::ShrEq     => (0, InfixKind::Assign(AssignOp::ShrEq)),

                // logical/bitwise
                Token::LogicalOr   => (1, InfixKind::Binary(BinOp::Or)),
                Token::LogicalAnd  => (2, InfixKind::Binary(BinOp::And)),
                Token::BitOr       => (3, InfixKind::Binary(BinOp::BitOr)),
                Token::BitXor      => (4, InfixKind::Binary(BinOp::BitXor)),
                Token::BitAnd      => (5, InfixKind::Binary(BinOp::BitAnd)),
                Token::EqEq        => (6, InfixKind::Binary(BinOp::Eq)),
                Token::NotEq       => (6, InfixKind::Binary(BinOp::NotEq)),

                // comparators
                Token::Less | Token::LessEq | Token::Greater | Token::GreaterEq => match tok {
                    Token::Less      => (7, InfixKind::Binary(BinOp::Less)),
                    Token::LessEq    => (7, InfixKind::Binary(BinOp::LessEq)),
                    Token::Greater   => (7, InfixKind::Binary(BinOp::Greater)),
                    Token::GreaterEq => (7, InfixKind::Binary(BinOp::GreaterEq)),
                    _ => unreachable!("what"),
                },

                // then comes assign its first match
                Token::Assign    => (0, InfixKind::Assign(AssignOp::Assign)),

                // bit shifts
                Token::Shl | Token::Shr => match tok {
                    Token::Shl => (8, InfixKind::Binary(BinOp::Shl)),
                    Token::Shr => (8, InfixKind::Binary(BinOp::Shr)),
                    _ => unreachable!("huh"),
                },

                // AS
                Token::Plus | Token::Minus => match tok {
                    Token::Plus  => (9, InfixKind::Binary(BinOp::Add)),
                    Token::Minus => (9, InfixKind::Binary(BinOp::Sub)),
                    _ => unreachable!("what the helly"),
                },

                // MD (m = mult AND modulo)
                Token::Star | Token::Slash | Token::Percent => match tok {
                    Token::Star    => (10, InfixKind::Binary(BinOp::Mul)),
                    Token::Slash   => (10, InfixKind::Binary(BinOp::Div)),
                    Token::Percent => (10, InfixKind::Binary(BinOp::Mod)),
                    _ => unreachable!("what the helliante"),
                },

                // erm
                _ => break,
            };

            // let higher precedence ops finish first
            if op_prec < min { break; }
            self.advance();

            match op {
                InfixKind::Assign(aop) => {
                    // assignments come last. otherwise left assoc
                    let rhs: Expr<'_> = self.parse_expr(op_prec);

                    let lhs = match left {
                        Expr::Ident(name) => LeftSide::Var(Ident(name)),
                        Expr::Field { obj, name } => LeftSide::Field { obj, name: Ident(name) },
                        Expr::Index { obj, sub } => LeftSide::Subscript { obj, sub },
                        _ => {
                            println!("not implemented: or something went wrong {tok:?}");
                            return Expr::Unknown;
                        }
                    };

                    left = Expr::Assign { op: aop, lhs, rhs: Box::new(rhs) };
                }

                InfixKind::Binary(bop) => {
                    let rhs: Expr<'_> = self.parse_expr(op_prec + 1);
                    left = Expr::Binary { op: bop, lhs: Box::new(left), rhs: Box::new(rhs) };
                }
            }
        }

        left
    }

    #[inline]
    fn parse_prefix(&mut self) -> Expr<'src> {
        // TODO: write proper error handling... and parse expr... and test this
        let tok: &Token<'_> = self.advance().expect("unexpected EOF");
        match tok {
            Token::Minus       => Expr::Unary { op: UnaryOp::Neg, expr: Box::new(self.parse_expr(12)) },
            Token::LogicalNot  => Expr::Unary { op: UnaryOp::Not, expr: Box::new(self.parse_expr(12)) },
            Token::BitNot      => Expr::Unary { op: UnaryOp::BitNot, expr: Box::new(self.parse_expr(12)) },
        
            Token::Identifier(name) => Expr::Ident(name),
            Token::LitInteger(n)    => Expr::Literal(Literal::Int(n)),
            Token::LitFloat(n)      => Expr::Literal(Literal::Float(n)),
            Token::LitString(s)     => Expr::Literal(Literal::String(s)),
            Token::LitChar(c)       => Expr::Literal(Literal::Char(c)),
            Token::Bool(b)          => Expr::Literal(Literal::Bool(*b)),
        
            // Token::LParen => {
            //     let e = self.parse_expr(0);
            //     self.expect(|t| matches!(t, Token::RParen)).expect("missing ')'");
            //     e
            // }
        
            // Token::If => self.parse_if_expr(),
            // Token::While => self.parse_while_expr(),
            // Token::Match => self.parse_match_expr(),
            // Token::LBrace => self.parse_block_expr(),
        
            // temporary solution for nimpl, i need to link ariadne
            _ => {
                // add this back w a debug flag idk if debug { println!("not implemented: {tok:?}"); }
                Expr::Unknown
            }
        }
    }

    pub fn parse(&mut self, debug: bool) -> Result<Vec<Stmt<'src>>, Vec<Diagnostic<'t, 'src>>> {
        let mut nodes: Vec<Stmt<'src>> = Vec::new();
        let start: Instant = Instant::now();
        if debug { println!(); }

        while let Some(cur) = self.cur() {
            match cur {
                // idents (read parse_ident)
                Token::Identifier(_) => nodes.push(Stmt::Expr(self.parse_expr(0))),

                // control flow
                Token::Break => nodes.push(Stmt::Break),
                Token::Continue => nodes.push(Stmt::Continue),

                // TODO: wire this to the SyntaxError setup i alr have
                _ => {
                    if debug { println!("not implemented: {cur}"); }
                    self.advance();
                    continue;
                }
            }

            if debug {
                println!("Parsed: \n{:#?}", nodes.last().unwrap());
            }
            self.advance();
        }

        println!(
            "Parsed {} tokens into {} nodes. Took {}s.",
            self.tokens.len(),
            nodes.len(),
            start.elapsed().as_secs_f64()
        );
        Ok(nodes)
    }
}

impl<'src, 't> fmt::Display for Parser<'src, 't> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}