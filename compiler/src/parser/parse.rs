// uncomment when this inevitably becomes a problem again #![allow(dead_code, unused_variables)]

use std::{mem::take, ops::Range, process::exit, time::Instant};

use super::ast::*;
use crate::error::{Diagnostic, ParseError::*, SyntaxError, SyntaxError::*};
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
    /// simple span helper
    fn cur_span(&self) -> Range<usize> {
        self.spans
            .get(self.pos)
            .cloned()
            .unwrap_or(self.src.len()..self.src.len())
    }

    #[inline]
    /// check the current token without advancing
    fn cur(&self) -> Option<&'t Token<'src>> {
        self.tokens.get(self.pos)
    }

    #[inline]
    /// check the following token without advancing
    fn peek(&self) -> Option<&'t Token<'src>> {
        self.tokens.get(self.pos + 1)
    }

    #[inline]
    /// check if the current token matches something without advancing
    fn matches(&self, matched: &Token<'src>) -> bool {
        let tok: &Token<'_> = self.cur().unwrap_or(&Token::Error);
        tok == matched
    }

    #[inline]
    /// check if any token in a slice is matched to the current token.
    /// matches any using a slice to reduce the amt of calls to self.matches
    fn matches_any(&self, any: &[Token<'src>]) -> bool {
        let tok: &Token<'_> = self.cur().unwrap_or(&Token::Error);
        any.contains(tok)
    }

    #[inline]
    /// push a diagnostic to the error vector
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

    // expect a token or return none if not there
    // #[inline]
    // fn expect<F>(&mut self, f: F) -> Option<&Token<'src>>
    // where
    //     F: FnOnce(&Token<'_>) -> bool,
    // {
    //     let tok = self.cur()?;
    //     if f(tok) {
    //         self.pos += 1;
    //         Some(tok)
    //     } else {
    //         None
    //     }
    // }

    #[inline]
    /// expect but with an error attached to it
    fn expect_msg<F>(&mut self, f: F, msg: &'src str) -> Option<&Token<'src>>
    where
        F: FnOnce(&Token<'_>) -> bool,
    {
        let tok = self.cur()?;
        if f(tok) {
            self.pos += 1;
            Some(tok)
        } else {
            self.error(Parse(MissingExpected(msg)));
            None
        }
    }

    #[inline]
    /// helper function to check if a modifier exists (wont advance if it isnt there)
    fn check_modifier(&mut self, token: &Token<'src>) -> bool {
        if self.matches(token) {
            self.advance();
            true
        } else {
            false
        }
    }

    #[inline]
    /// advance by a single token
    fn advance(&mut self) -> Option<&Token<'src>> {
        self.advance_by(1)
    }

    #[inline]
    /// advance by n tokens
    fn advance_by(&mut self, n: u8) -> Option<&Token<'src>> {
        let tok: &Token<'src> = self.cur()?;
        self.pos += n as usize;
        Some(tok)
    }

    #[inline]
    // eat current identifier and hook lexer span to the node
    fn take_ident(&mut self) -> Option<Ident<'src>> {
        match self.cur() {
            Some(Token::Identifier(name)) => {
                let ident = Ident(name, self.cur_span());
                self.advance();
                Some(ident)
            }
            _ => None,
        }
    }

    #[inline]
    /// skip all newlines following a statement
    fn eat_newlines(&mut self) {
        while self.matches(&Token::Newline) {
            self.advance();
        }
    }

    #[inline]
    // skip any delimiter between a statement (i need to fix this to avoid semicolon spam)
    fn eat_stmt_delimiters(&mut self) {
        while self.matches_any(&[Token::Newline, Token::Semicolon]) {
            self.advance();
        }
    }

    #[inline]
    /// parse if we havent reached one of the listed stops (prototypes vs declarations)
    fn parse_optional_expr_until(&mut self, stops: &[Token<'src>]) -> Option<Box<Expr<'src>>> {
        if self.matches_any(stops) {
            None
        } else {
            Some(Box::new(self.parse_expr(0)))
        }
    }

    fn parse_call_args(&mut self) -> Vec<Expr<'src>> {
        let mut args = Vec::with_capacity(8);
        if !self.matches(&Token::RParen) {
            args.push(self.parse_expr(0));

            while self.matches(&Token::Comma) {
                self.advance();
                args.push(self.parse_expr(0));
                if self.matches(&Token::RParen) {
                    break;
                }
            }

            if !self.matches(&Token::RParen) {
                self.error(Parse(MissingExpected(
                    "expected ',' or ')' in call. have to add this to the error system",
                )));
            }
        }

        args
    }

    fn parse_subscript(&mut self) -> Subscript<'src> {
        if self.matches(&Token::DotDot) {
            self.advance();
            let end = self.parse_optional_expr_until(&[Token::RBracket]);
            return Subscript::Range { start: None, end };
        }

        let start = self.parse_expr(0);
        if self.matches(&Token::DotDot) {
            self.advance();
            let end = self.parse_optional_expr_until(&[Token::RBracket]);
            Subscript::Range {
                start: Some(Box::new(start)),
                end,
            }
        } else {
            Subscript::Index(Box::new(start))
        }
    }

    fn postfix_precedence(&self) -> u8 {
        match self.cur() {
            Some(
                Token::LParen
                | Token::Unit // every language of mine has one fucky thing... unit is used for calls
                | Token::LBracket
                | Token::Dot
                | Token::Arrow
                | Token::PlusPlus
                | Token::MinusMinus,
            ) => 15,
            _ => 0,
        }
    }

    fn apply_postfix(&mut self, left: &mut Expr<'src>) {
        let current = std::mem::replace(left, Expr::Unknown);

        if self.matches_any(&[Token::LParen, Token::Unit]) {
            let args = if self.matches(&Token::LParen) {
                self.advance();
                let args = self.parse_call_args();
                self.expect_msg(
                    |t: &Token<'_>| matches!(t, Token::RParen),
                    "expected ')' to close function call",
                );
                args
            } else {
                self.advance();
                Vec::new()
            };

            *left = match current {
                Expr::Field { obj, name } => Expr::Method {
                    receiver: obj,
                    method: name,
                    args,
                },
                other => Expr::Call {
                    func: Box::new(other),
                    args,
                },
            };
            return;
        }

        if self.matches_any(&[Token::Dot, Token::Arrow]) {
            self.advance();
            let name = match self.take_ident() {
                Some(name) => name,
                _ => {
                    self.error(Parse(MissingExpected(
                        "expected identifier after field access operator",
                    )));
                    *left = Expr::Unknown;
                    return;
                }
            };

            *left = Expr::Field {
                obj: Box::new(current),
                name,
            };
            return;
        }

        if self.matches(&Token::LBracket) {
            self.advance();
            let sub = self.parse_subscript();
            self.expect_msg(|t: &Token<'_>| matches!(t, Token::RBracket), "missing ']'");
            *left = Expr::Index {
                obj: Box::new(current),
                sub,
            };
            return;
        }

        if self.matches(&Token::PlusPlus) {
            self.advance();
            *left = Expr::Unary {
                op: UnaryOp::PostInc,
                expr: Box::new(current),
            };
            return;
        }

        if self.matches(&Token::MinusMinus) {
            self.advance();
            *left = Expr::Unary {
                op: UnaryOp::PostDec,
                expr: Box::new(current),
            };
            return;
        }

        *left = current;
    }

    fn infix_op(&self) -> Option<(u8, InfixKind)> {
        let tok = self.cur()?;

        Some(match tok {
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
            Token::LogicalOr => (1, InfixKind::Binary(BinOp::Or)),
            Token::LogicalAnd => (2, InfixKind::Binary(BinOp::And)),
            Token::BitOr => (3, InfixKind::Binary(BinOp::BitOr)),
            Token::BitXor => (4, InfixKind::Binary(BinOp::BitXor)),
            Token::BitAnd => (5, InfixKind::Binary(BinOp::BitAnd)),
            Token::EqEq => (6, InfixKind::Binary(BinOp::Eq)),
            Token::NotEq => (6, InfixKind::Binary(BinOp::NotEq)),
            Token::Less => (7, InfixKind::Binary(BinOp::Less)),
            Token::LessEq => (7, InfixKind::Binary(BinOp::LessEq)),
            Token::Greater => (7, InfixKind::Binary(BinOp::Greater)),
            Token::GreaterEq => (7, InfixKind::Binary(BinOp::GreaterEq)),
            Token::Assign => (0, InfixKind::Assign(AssignOp::Assign)),
            Token::Shl => (8, InfixKind::Binary(BinOp::Shl)),
            Token::Shr => (8, InfixKind::Binary(BinOp::Shr)),
            Token::Plus => (9, InfixKind::Binary(BinOp::Add)),
            Token::Minus => (9, InfixKind::Binary(BinOp::Sub)),
            Token::Star => (10, InfixKind::Binary(BinOp::Mul)),
            Token::Slash => (10, InfixKind::Binary(BinOp::Div)),
            Token::Percent => (10, InfixKind::Binary(BinOp::Mod)),
            Token::StarStar => (11, InfixKind::Binary(BinOp::Power)),
            _ => return None,
        })
    }

    fn assign_lhs(&mut self, expr: Expr<'src>) -> Option<LeftSide<'src>> {
        match expr {
            Expr::Ident(ident) => Some(LeftSide::Var(ident)),
            Expr::Field { obj, name } => Some(LeftSide::Field { obj, name }),
            Expr::Index { obj, sub } => Some(LeftSide::Subscript { obj, sub }),
            _ => {
                self.error(Parse(MissingExpected(
                    "left side of assignment must be identifier, field, or index",
                )));
                None
            }
        }
    }

    #[inline]
    fn parse_expr(&mut self, min: u8) -> Expr<'src> {
        let mut left = self.parse_prefix();

        while self.cur().is_some() {
            let precedence = self.postfix_precedence();
            if precedence != 0 && precedence >= min {
                self.apply_postfix(&mut left);
                continue;
            }

            if self.matches(&Token::DotDot) {
                self.advance();
                let end = self.parse_optional_expr_until(&[
                    Token::LBrace,
                    Token::Newline,
                    Token::Semicolon,
                    Token::RBracket,
                    Token::RParen,
                ]);

                left = Expr::Range {
                    start: Some(Box::new(left)),
                    end,
                };
                continue;
            }

            let (op_prec, op) = match self.infix_op() {
                Some(op) => op,
                None => break,
            };

            if op_prec < min {
                break;
            }
            self.advance();

            left = match op {
                InfixKind::Assign(aop) => {
                    let rhs = self.parse_expr(op_prec);
                    let lhs = match self.assign_lhs(left) {
                        Some(lhs) => lhs,
                        None => return Expr::Unknown,
                    };

                    Expr::Assign {
                        op: aop,
                        lhs,
                        rhs: Box::new(rhs),
                    }
                }

                InfixKind::Binary(bop) => {
                    let rhs = self.parse_expr(op_prec + 1);
                    Expr::Binary {
                        op: bop,
                        lhs: Box::new(left),
                        rhs: Box::new(rhs),
                    }
                }
            };
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

                let end = self.parse_optional_expr_until(&[
                    Token::LBrace,
                    Token::Newline,
                    Token::Semicolon,
                ]);

                Pattern::Range { start: None, end }
            }

            Some(Token::Identifier(name)) => {
                let _ = name;
                Pattern::Ident(
                    self.take_ident()
                        .expect("identifier disappeared while parsing pattern"),
                )
            }

            // tuples
            Some(Token::LParen) => {
                self.advance();
                let mut patterns = vec![self.parse_pattern()];

                while self.matches(&Token::Comma) {
                    self.advance();
                    patterns.push(self.parse_pattern());
                }

                self.expect_msg(
                    |t| matches!(t, Token::RParen),
                    "expected ')' in tuple pattern",
                );

                Pattern::Tuple(patterns)
            }

            // default
            _ => Pattern::Wildcard,
        }
    }

    fn parse_func(&mut self) -> Result<Stmt<'src>, SyntaxError<'src>> {
        self.advance();
        let name = match self.take_ident() {
            Some(name) => name,
            _ => {
                return Err(Parse(MissingExpected(
                    "function decl requires a name... did you forget it",
                )));
            }
        };

        // syntax: fn func(arg: type, arg2: type) -> rtntype { }
        // unit type is colliding so if no args it's js the unit
        let has_args = self.matches(&Token::LParen);
        if !self.matches_any(&[Token::LParen, Token::Unit]) {
            self.error(Parse(MissingExpected(
                "missing () around function args in definition",
            )));
        }

        // you can use newlines to make ur args cleaner too
        self.advance();
        self.eat_newlines();

        // TODO: make this use self.eat_args when its made
        let mut args: Vec<(Ident<'src>, Type<'src>)> = Vec::with_capacity(8);
        if has_args {
            while !self.matches(&Token::RParen) {
                let argname = match self.take_ident() {
                    Some(name) => name,
                    _ => {
                        return Err(Parse(MissingExpected(
                            "arguments also require a name... did you forget them",
                        )));
                    }
                };

                self.expect_msg(
                    |t: &Token<'_>| matches!(t, Token::Colon),
                    "arguments need to be typed, or specifically denoted compiler inferred (likely missing ':' after argument name)"
                );

                let argtyp: Type<'_> = self.parse_type();
                args.push((argname, argtyp));

                self.eat_newlines();

                if !self.matches_any(&[Token::Comma, Token::RParen]) {
                    return Err(Parse(MissingExpected(
                        "arguments must be seperated by commas in the definition (may not have that be the case for calls)",
                    )));
                } else if self.matches(&Token::Comma) && self.peek() != Some(&Token::RParen) {
                    self.advance();
                }
            }

            self.expect_msg(
                |t: &Token<'_>| matches!(t, Token::RParen),
                "missing closing parenthesis around function args in definition",
            );

            // for all u that do the arrow on the other line too... see below
            self.eat_newlines();
        }

        self.expect_msg(
            |t: &Token<'_>| matches!(t, Token::Arrow),
            "functions must contain an explicit return type",
        );

        // noticing how this is unnecessarily long now
        let typ = self.parse_type();

        // semicolon has to be on the same line
        let body = if self.matches(&Token::Semicolon) {
            None
        } else {
            // brace does not for all you headasses that do
            // fn a () -> _
            // {
            self.eat_newlines();

            // check for a brace before parsing, otherwise malformed
            let lbrace: bool = self.expect_msg(
                |t| matches!(t, Token::LBrace),
                "function definitions must either end in a semicolon to show it's a prototype, or a brace initiating the body",
            ).is_some();

            Some(if lbrace {
                self.parse_block_expr()
            } else {
                Expr::Unknown
            })
        };

        // TODO: self.eat_args
        Ok(Stmt::FuncDecl {
            name,
            typ,
            args,
            body,
        })
    }

    fn parse_for_expr(&mut self) -> Expr<'src> {
        let pattern: Pattern<'_> = self.parse_pattern();

        // syntax: for _ in r1..r2 (TODO figure out step amts)
        self.expect_msg(
            |t: &Token<'_>| matches!(t, Token::In),
            "missing keyword 'in' inside for loop",
        );

        let iter = self.parse_expr(0);

        // for loops need braces for right now (TODO one line for loops)
        self.expect_msg(
            |t: &Token<'_>| matches!(t, Token::LBrace),
            "expected '{' in for loop",
        );

        let body: Expr<'_> = self.parse_block_expr();

        Expr::For {
            pattern,
            iter: Box::new(iter),
            body: Box::new(body),
        }
    }

    fn parse_if_expr(&mut self) -> Expr<'src> {
        let cond: Expr<'_> = self.parse_expr(0);
        if self
            .expect_msg(
                |t: &Token<'_>| matches!(t, Token::LBrace),
                "missing '{' before if body",
            )
            .is_none()
        {
            return Expr::Unknown;
        }

        let then: Expr<'_> = self.parse_block_expr();

        // ONLY eat newlines when there's an else clause, otherwise the parser needs it as a delimiter
        let checkpoint: usize = self.pos;
        self.eat_newlines();

        // check for an else statement, and skip present newlines
        let else_: Option<Box<Expr<'_>>> = if self.matches(&Token::Else) {
            self.advance();
            self.eat_newlines();

            // recursively parse as an Else { If {} } we have an else if
            if self.matches(&Token::If) {
                self.advance();
                Some(Box::new(self.parse_if_expr()))
            }
            // otherwise make sure that brace is there (and then parse)
            else if self
                .expect_msg(
                    |t: &Token<'_>| matches!(t, Token::LBrace),
                    "missing '{' before else body",
                )
                .is_none()
            {
                None
            } else {
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

    fn parse_while_expr(&mut self) -> Expr<'src> {
        let cond: Expr<'_> = self.parse_expr(0);
        if self
            .expect_msg(
                |t: &Token<'_>| matches!(t, Token::LBrace),
                "missing '{' before while body",
            )
            .is_none()
        {
            return Expr::Unknown;
        }

        let body: Expr<'_> = self.parse_block_expr();

        Expr::While {
            cond: Box::new(cond),
            body: Box::new(body),
        }
    }

    fn parse_match_expr(&mut self) -> Expr<'src> {
        self.advance();

        // an expression is what gets matched (whether that be a value or something that evaluates to a value)
        let item = self.parse_expr(0);

        // matches are surrounded with braces
        if self
            .expect_msg(
                |t| matches!(t, &Token::LBrace),
                "expected '{' before match arms",
            )
            .is_none()
        {
            return Expr::Unknown;
        }

        let mut branches = Vec::new();

        while !self.matches(&Token::RBrace) {
            // skip newlines and check for closer
            self.eat_newlines();
            if self.matches(&Token::RBrace) {
                break;
            }

            // parse pattern then expect ->
            let pattern = self.parse_pattern();
            if self
                .expect_msg(
                    |t| matches!(t, &Token::Arrow),
                    "expected '->' after pattern in match arm",
                )
                .is_none()
            {
                break;
            }

            // parse body (either a single expr or a block)
            let body = if self.matches(&Token::LBrace) {
                Stmt::Expr(self.parse_block_expr())
            } else {
                Stmt::Expr(self.parse_expr(0))
            };

            // optional guard when i'm not too fuckin lazy to add them
            let guard = None;

            branches.push(Branch {
                pattern,
                guard,
                body,
            });

            // handle delimiters between stuff
            self.eat_stmt_delimiters();
        }

        self.expect_msg(
            |t| matches!(t, &Token::RBrace),
            "expected '}' to close match expression",
        );

        Expr::Match {
            item: Box::new(item),
            branches,
        }
    }

    fn parse_type(&mut self) -> Type<'src> {
        // TODO: add support for array and generic types
        let typ: Type<'src> = match self.expect_msg(
            |t| matches!(t, &Token::Identifier(_) | &Token::Unit | &Token::Underscore),
            "invalid or missing type",
        ) {
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
                _ => Type::Ident(Ident(typname, self.spans[self.pos - 1].clone())),
            },

            // unit type and inferred have to be handled seperately
            Some(Token::Unit) => Type::Unit,
            Some(Token::Underscore) => Type::Inferred,

            // push missing type after :
            _ => {
                // TODO: potentially borrow parse_type as immutable and deal with the error outside
                // this way we can have seperate errors for function type and variable type
                Type::Error
            }
        };

        // durrrrr make type the tail obviously
        typ
    }

    fn parse_block_expr(&mut self) -> Expr<'src> {
        let mut stmts: Vec<Stmt<'src>> = Vec::new();
        let mut tail: Option<Box<Expr<'src>>> = None;

        loop {
            let tok = match self.cur() {
                Some(t) => t,

                // no closing } errors out
                None => {
                    self.error(Parse(MissingExpected("expected '}' to close block")));
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

                Token::Fn => match self.parse_func() {
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
                    self.eat_newlines();

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
                self.eat_stmt_delimiters();
                continue;
            }

            if self.matches(&Token::RBrace) {
                self.advance();
                break;
            }

            self.error(Parse(MissingExpected(
                "expected ';', newline, or '}' after statement in block",
            )));
        }

        Expr::Block { stmts, tail }
    }

    #[inline]
    fn parse_prefix(&mut self) -> Expr<'src> {
        // TODO: write proper error handling... and parse expr... and test this
        let span = self.cur_span();
        let tok: &Token<'_> = match self.advance() {
            Some(t) => t,
            None => {
                self.error(Parse(MissingExpected("unexpected EOF")));
                return Expr::Unknown;
            }
        };
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
                let end = self.parse_optional_expr_until(&[
                    Token::LBrace,
                    Token::Newline,
                    Token::Semicolon,
                    Token::RBracket,
                    Token::RParen,
                ]);

                Expr::Range { start: None, end }
            }

            Token::LParen => {
                let inner = self.parse_expr(0);
                self.expect_msg(
                    |t: &Token<'_>| matches!(t, Token::RParen),
                    "expected ')' to close parenthesized expression",
                );
                inner
            }

            Token::Identifier(name) => Expr::Ident(Ident(name, span)),
            Token::LitInteger(n) => Expr::Literal(Literal::Int(n, span)),
            Token::LitFloat(n) => Expr::Literal(Literal::Float(n, span)),
            Token::LitString(s) => Expr::Literal(Literal::String(s, span)),
            Token::LitChar(c) => Expr::Literal(Literal::Char(c, span)),
            Token::Bool(b) => Expr::Literal(Literal::Bool(*b, span)),
            Token::Unit => Expr::Literal(Literal::Unit(span)),

            Token::If => self.parse_if_expr(),
            Token::While => self.parse_while_expr(),
            Token::For => self.parse_for_expr(),
            Token::Match => self.parse_match_expr(),

            Token::LBrace => self.parse_block_expr(),

            _ => {
                // add this back w a debug flag idk if debug { println!("not implemented: {tok:?}"); }
                Expr::Unknown
            }
        }
    }

    pub fn parse_let(&mut self) -> Result<Stmt<'src>, SyntaxError<'src>> {
        self.advance();

        // specifiers are evaluated in this order
        let constant = self.check_modifier(&Token::Const);
        let global = self.check_modifier(&Token::Static);
        let mutable = self.check_modifier(&Token::Mutable);

        // ensure constant isnt used where it can't be
        if constant && mutable {
            self.error(Parse(ConstDisallowed(
                "constant cannot be used in tandem with mutable.",
            )));
        }
        if constant && global {
            self.error(Parse(ConstDisallowed(
                "constant cannot be used in tandem with static.",
            )));
        }

        // consume name (TODO: add let _)
        let name: Ident<'_> = match self.take_ident() {
            Some(name) => name,
            _ => {
                return Err(Parse(MissingExpected(
                    "let must have an identifier afterwards",
                )));
            }
        };

        // parse type annotation (if its there)
        let typ = if self.matches(&Token::Colon) {
            self.advance();
            self.parse_type()
        } else {
            Type::Inferred
        };

        // parse initializer (also dependent on if its there)
        let mut init = None;
        if self.matches(&Token::Assign) {
            self.advance();

            match self.cur().unwrap_or(&Token::Error) {
                Token::Error | Token::Newline | Token::Semicolon | Token::Eof => {
                    return Err(Parse(MissingExpected("expected expression after '='")));
                }

                _ => {
                    init = Some(self.parse_expr(0));
                }
            }
        }

        // can't automatically deduce type on assignment (maybe make it so that the type is filled when assigned to?)
        if typ == Type::Inferred && init.is_none() {
            return Err(Parse(MissingExpected(
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

                Token::Let => match self.parse_let() {
                    Ok(stmt) => nodes.push(stmt),
                    Err(e) => self.error(e),
                },

                // same dispatch for parse func
                Token::Fn => match self.parse_func() {
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
                self.error(Parse(MissingExpected(
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
