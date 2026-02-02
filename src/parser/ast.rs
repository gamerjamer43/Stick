// keeping this on until everything in the parser is done...
#![allow(dead_code)]

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ident<'src>(pub &'src str);

// make ident print as just the value inside
impl<'src> fmt::Display for Ident<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// literals for all the types below
#[derive(Debug, Clone, PartialEq)]
pub enum Literal<'src> {
    // TODO: figure out if i should just do Int with a bit value at parse
    // or if i should just store seperate literals for each bit width
    Int(&'src str),
    Uint(&'src str),

    Float(&'src str),
    Double(&'src str),
    Bool(bool),
    Char(&'src str),
    String(&'src str),
    Unit,
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

/// array accesses should only be indexing or slicing
#[derive(Debug, Clone, PartialEq)]
pub enum Subscript<'src> {
    Index(Box<Expr<'src>>),
    Range {
        start: Option<Box<Expr<'src>>>,
        end: Option<Box<Expr<'src>>>,
    },
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
        body: Option<Box<Expr<'src>>>,
    },

    Match {
        item: Box<Expr<'src>>,
        branches: Vec<Branch<'src>>,
    },

    // name for enhanced for loops, will just be iter if not
    For {
        name: &'src str,
        iter: Box<Expr<'src>>,
        body: Box<Stmt<'src>>,
    },

    Unknown,
}

/// helper for the specific thing matched on a pattern match
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern<'src> {
    /// wildcard/default match
    Wildcard,

    /// just a plain identifier which binds its value
    Ident(&'src str),

    /// literal value match
    Literal(Literal<'src>),

    // match multiple cases
    Or(Vec<Pattern<'src>>),

    // interval matching (1..10 or similar)
    Range {
        start: Option<Box<Expr<'src>>>,
        end: Option<Box<Expr<'src>>>,
    },

    // shit i have to add later
    // Tuple(Vec<Pattern<'src>>),
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
}
