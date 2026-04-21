// keeping this on until everything in the parser is done...
#![allow(dead_code)]

use core::fmt;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident<'src>(pub &'src str, pub Range<usize>);

impl<'src> Ident<'src> {
    pub fn span(&self) -> Range<usize> {
        self.1.clone()
    }
}

// make ident print as just the value inside
impl<'src> fmt::Display for Ident<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// literals for all the types below
#[derive(Debug, Clone, PartialEq)]
pub enum Literal<'src> {
    Int(&'src str, Range<usize>),    // ints coerce to i64
    Float(&'src str, Range<usize>),  // floats coerce to f64
    Bool(bool, Range<usize>),        // boolean syntax === rusts
    Char(&'src str, Range<usize>),   // chars will be a scalar like rust
    String(&'src str, Range<usize>), // utf8 strings (because why utf32)
    Unit(Range<usize>),              //
}

// store spans with them
impl<'src> Literal<'src> {
    pub fn span(&self) -> Range<usize> {
        match self {
            Literal::Int(_, span)
            | Literal::Float(_, span)
            | Literal::Bool(_, span)
            | Literal::Char(_, span)
            | Literal::String(_, span)
            | Literal::Unit(span) => span.clone(),
        }
    }
}

fn join_spans(lhs: Range<usize>, rhs: Range<usize>) -> Range<usize> {
    lhs.start.min(rhs.start)..lhs.end.max(rhs.end)
}

/// all builtin types
#[derive(Debug, Clone, PartialEq)]
pub enum Type<'src> {
    // one byte
    I8,
    U8,
    Bool,
    Char,

    // two byte
    I16,
    U16,

    // four byte
    I32,
    U32,
    F32,

    // eight byte
    I64,
    U64,
    F64,

    // void/unit
    Unit,

    // string type (NOT THE LITERAL)
    Str,

    // user defined types (local to scope)
    Ident(Ident<'src>),

    /// `lib`, `std::io::File`, maybe others
    Path(Vec<Ident<'src>>),

    /// fixed size, dynamic type, immutable
    Tuple(Vec<Type<'src>>),

    /// fixed size, static type, mutable
    Array {
        typ: Box<Type<'src>>,
        len: Option<u64>,
    },

    /// polish dictionary defines function as: "everyone knows what a function is"
    Func {
        params: Vec<Type<'src>>,
        ret: Box<Type<'src>>,
    },

    /// variadic function parameter (...T)
    VarArgs(Box<Type<'src>>),

    // if i add a borrow system
    // `&T` / `&mut T`
    // Ref {
    //     mutable: bool,
    //     inner: Box<Type<'src>>,
    // },

    // `*T` / `*mut T`
    // Ptr {
    //     mutable: bool,
    //     inner: Box<Type<'src>>,
    // },

    // type unlisted, or specifically marked as inferred
    Inferred,

    // any type deduction errors
    Error,
}

/// a small list of everything that can be on the left hand side of an assignment
#[derive(Debug, Clone, PartialEq)]
pub enum LeftSide<'src> {
    // plain idents
    Var(Ident<'src>),

    // field (struct/obj.field)
    Field {
        obj: Box<Expr<'src>>,
        name: Ident<'src>,
    },

    // subscript (tuple/array[i] or [i..j]/[..i])
    Subscript {
        obj: Box<Expr<'src>>,
        sub: Subscript<'src>,
    },
}

impl<'src> LeftSide<'src> {
    pub fn span(&self) -> Range<usize> {
        match self {
            LeftSide::Var(ident) => ident.span(),
            LeftSide::Field { obj, name } => join_spans(obj.span(), name.span()),
            LeftSide::Subscript { obj, sub } => join_spans(obj.span(), sub.span()),
        }
    }
}

/// array accesses should only be indexing or slicing
#[derive(Debug, Clone, PartialEq)]
pub enum Subscript<'src> {
    Index(Box<Expr<'src>>),
    Range {
        start: Option<Box<Expr<'src>>>,
        end: Option<Box<Expr<'src>>>,
    },
}

impl<'src> Subscript<'src> {
    pub fn span(&self) -> Range<usize> {
        match self {
            Subscript::Index(expr) => expr.span(),
            Subscript::Range { start, end } => match (start, end) {
                (Some(start), Some(end)) => join_spans(start.span(), end.span()),
                (Some(start), None) => start.span(),
                (None, Some(end)) => end.span(),
                (None, None) => 0..0,
            },
        }
    }
}

/// both types of operation that can be infixed
#[derive(Debug)]
pub enum InfixKind {
    Binary(BinOp),
    Assign(AssignOp),
}

/// all binary operators provided natively
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Power,

    // equality operations
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,

    // logical operations
    And,
    Or,

    // bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// and the 3 unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    PreInc,
    PreDec,
    PostInc,
    PostDec,
    Neg,
    Not,
    BitNot,
}

/// those same operators but assignment... 1 to 1 mapping frm tokens. this does require a copy tho
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    // basic assignment
    Assign,

    // arithmetic assignment
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,

    // bitwise assignment
    AndEq,
    OrEq,
    XorEq,
    ShlEq,
    ShrEq,
}

/// general expressions which will be recursively parsed using chumsky
/// box anything recursive, as otherwise the enum will be infinite, and rust needs
/// to know the size at compile time.
/// TODO: implement constant expressions (ConstExpr) which are evaluated down to a fixed integer value
/// TODO 2: small but figure out how to avoid all that boxing. one option is using an arena but das kinda OD
#[derive(Debug, Clone, PartialEq)]
pub enum Expr<'src> {
    // var names
    Ident(Ident<'src>),

    // literal values
    Literal(Literal<'src>),

    // assignments
    Assign {
        op: AssignOp,
        lhs: LeftSide<'src>,
        rhs: Box<Expr<'src>>,
    },

    // unary operations
    Unary {
        op: UnaryOp,
        expr: Box<Expr<'src>>,
    },

    // binary ops
    Binary {
        op: BinOp,
        lhs: Box<Expr<'src>>,
        rhs: Box<Expr<'src>>,
    },

    // function call
    Call {
        func: Box<Expr<'src>>,
        args: Vec<Expr<'src>>,
    },

    // field access (a.b)
    Field {
        obj: Box<Expr<'src>>,
        name: Ident<'src>,
        // TODO: figure out ->
        // whether this access is a pointer or not
        // ptr: bool,

        // whether the compiler should automatically deref this value or not (not implemented yet so always false)
        // deref: bool,
    },

    // method calls (a.b(); a.b().c(); a().b() and etc.)
    Method {
        // receiver isnt always just an obj, it can be a chained call
        receiver: Box<Expr<'src>>,
        method: Ident<'src>,
        args: Vec<Expr<'src>>,
    },

    // index or slice (a[b] or a[b..c])
    Index {
        obj: Box<Expr<'src>>,
        sub: Subscript<'src>,
    },

    // a scoped block, used for any sort of "statement".
    // stores the instructions inside and what it evaluates to (or none)
    Block {
        stmts: Vec<Stmt<'src>>,
        tail: Option<Box<Expr<'src>>>,
    },

    // control flow
    If {
        cond: Box<Expr<'src>>,
        then: Box<Expr<'src>>,

        // another if can get fed into here
        else_: Option<Box<Expr<'src>>>,
    },

    While {
        cond: Box<Expr<'src>>,
        body: Box<Expr<'src>>,
    },

    Match {
        item: Box<Expr<'src>>,
        branches: Vec<Branch<'src>>,
    },

    // name for enhanced for loops, will just be iter if not
    For {
        pattern: Pattern<'src>,
        iter: Box<Expr<'src>>,
        body: Box<Expr<'src>>,
    },

    // standalone range expressions (0..5, ..5, 0..)
    Range {
        start: Option<Box<Expr<'src>>>,
        end: Option<Box<Expr<'src>>>,
    },

    Unknown,
}

impl<'src> Expr<'src> {
    pub fn span(&self) -> Range<usize> {
        match self {
            Expr::Ident(ident) => ident.span(),
            Expr::Literal(literal) => literal.span(),
            Expr::Assign { lhs, rhs, .. } => join_spans(lhs.span(), rhs.span()),
            Expr::Unary { expr, .. } => expr.span(),
            Expr::Binary { lhs, rhs, .. } => join_spans(lhs.span(), rhs.span()),
            Expr::Call { func, args } => args
                .last()
                .map(|arg| join_spans(func.span(), arg.span()))
                .unwrap_or_else(|| func.span()),
            Expr::Field { obj, name } => join_spans(obj.span(), name.span()),
            Expr::Method {
                receiver,
                method,
                args,
            } => args
                .last()
                .map(|arg| join_spans(receiver.span(), arg.span()))
                .unwrap_or_else(|| join_spans(receiver.span(), method.span())),
            Expr::Index { obj, sub } => join_spans(obj.span(), sub.span()),
            Expr::Block { stmts, tail } => tail
                .as_ref()
                .map(|expr| expr.span())
                .or_else(|| stmts.last().map(Stmt::span))
                .unwrap_or(0..0),
            Expr::If { cond, then, else_ } => else_
                .as_ref()
                .map(|else_expr| join_spans(cond.span(), else_expr.span()))
                .unwrap_or_else(|| join_spans(cond.span(), then.span())),
            Expr::While { cond, body } => join_spans(cond.span(), body.span()),
            Expr::Match { item, branches } => branches
                .last()
                .map(|branch| join_spans(item.span(), branch.body.span()))
                .unwrap_or_else(|| item.span()),
            Expr::For { iter, body, .. } => join_spans(iter.span(), body.span()),
            Expr::Range { start, end } => match (start, end) {
                (Some(start), Some(end)) => join_spans(start.span(), end.span()),
                (Some(start), None) => start.span(),
                (None, Some(end)) => end.span(),
                (None, None) => 0..0,
            },
            Expr::Unknown => 0..0,
        }
    }
}

/// helper for the specific thing matched on a pattern match
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern<'src> {
    /// wildcard/default match
    Wildcard,

    /// just a plain identifier which binds its value
    Ident(Ident<'src>),

    /// literal value match
    Literal(Literal<'src>),

    // match multiple cases
    Or(Vec<Pattern<'src>>),

    // interval matching (1..10 or similar)
    Range {
        start: Option<Box<Expr<'src>>>,
        end: Option<Box<Expr<'src>>>,
    },

    // tuples (i, j)
    Tuple(Vec<Pattern<'src>>),
    // shit i have to add later
    // Array
    //
    // will prolly expand but for rn this is ok
}

/// each branch of a match statement
#[derive(Debug, Clone, PartialEq)]
pub struct Branch<'src> {
    pub pattern: Pattern<'src>,
    pub guard: Option<Box<Expr<'src>>>,
    pub body: Stmt<'src>,
}

/// all types of statement. either control or a normal expression
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt<'src> {
    Expr(Expr<'src>),

    // control flow
    Return(Option<Expr<'src>>),
    Break,
    Continue,

    // variable declaration is a statement rather than an expression
    VarDecl {
        name: Ident<'src>,
        typ: Type<'src>,
        init: Option<Expr<'src>>,

        // may drop this, but adding immutability for like tuples
        // forces a reassignment to change so may keep this as it has its purpose
        mutable: bool,
        constant: bool,

        // global == static will prolly change that
        global: bool,
    },

    // same with functions
    FuncDecl {
        name: Ident<'src>,
        typ: Type<'src>,

        // list of arguments
        args: Vec<(Ident<'src>, Type<'src>)>,

        // if no body it's a function prototype
        body: Option<Expr<'src>>,
    },

    // improperly parsed statements
    Error,
}

impl<'src> Stmt<'src> {
    pub fn span(&self) -> Range<usize> {
        match self {
            Stmt::Expr(expr) => expr.span(),
            Stmt::Return(Some(expr)) => expr.span(),
            Stmt::VarDecl { name, init, .. } => init
                .as_ref()
                .map(|expr| join_spans(name.span(), expr.span()))
                .unwrap_or_else(|| name.span()),
            Stmt::FuncDecl { name, body, .. } => body
                .as_ref()
                .map(|expr| join_spans(name.span(), expr.span()))
                .unwrap_or_else(|| name.span()),
            Stmt::Return(None) | Stmt::Break | Stmt::Continue | Stmt::Error => 0..0,
        }
    }
}
