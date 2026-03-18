use std::collections::HashMap;

use crate::{
    error::Diagnostic,
    parser::ast::{AssignOp, BinOp, Expr, Literal, Stmt, Type, UnaryOp},
};

pub struct Analyzer<'a, 'src> {
    pub nodes: Vec<Stmt<'src>>,
    pub resolved: HashMap<&'src str, usize>, // usize contains a unique symbol id
    pub types: HashMap<&'src str, Type<'src>>,
    pub errors: Vec<Diagnostic<'a, 'src>>,
    pub pos: usize,
}

impl<'a, 'src> Analyzer<'a, 'src> {
    pub fn new(nodes: Vec<Stmt<'src>>) -> Self {
        Self {
            nodes,
            resolved: HashMap::new(),
            types: HashMap::new(),
            errors: Vec::new(),
            pos: 0,
        }
    }

    /// reused from parser: check the current node without advancing
    fn cur(&self) -> Option<&Stmt<'src>> {
        self.nodes.get(self.pos)
    }

    // numeric types have a precedence attached to them
    fn numeric_rank(typ: &Type<'src>) -> Option<u8> {
        match typ {
            Type::I8 | Type::U8 => Some(1),
            Type::I16 | Type::U16 => Some(2),
            Type::I32 | Type::U32 => Some(3),
            Type::I64 | Type::U64 => Some(4),
            Type::F32 => Some(5),
            Type::F64 => Some(6),
            _ => None,
        }
    }

    // confirms you can assign a type properly
    fn can_assign(&self, expected: &Type<'src>, actual: &Type<'src>) -> bool {
        if expected == actual {
            return true;
        }

        // god i love rust but i hate some of the patterns it makes...
        let same_family = matches!(
            (expected, actual),
            (
                Type::I8 | Type::I16 | Type::I32 | Type::I64,
                Type::I8 | Type::I16 | Type::I32 | Type::I64
            ) | (
                Type::U8 | Type::U16 | Type::U32 | Type::U64,
                Type::U8 | Type::U16 | Type::U32 | Type::U64
            ) | (Type::F32 | Type::F64, Type::F32 | Type::F64)
        );

        if !same_family {
            return false;
        }

        match (Self::numeric_rank(expected), Self::numeric_rank(actual)) {
            (Some(expected_rank), Some(actual_rank)) => expected_rank >= actual_rank,
            _ => false,
        }
    }

    // if type can be given a rank (and isnt a float) we can do bitwise ops
    fn is_bitwise_numeric(typ: &Type<'src>) -> bool {
        match Self::numeric_rank(typ) {
            Some(_) => !matches!(typ, Type::F32 | Type::F64),
            None => false,
        }
    }

    fn infer_unary_type(&self, op: &UnaryOp, inner: Type<'src>) -> Option<Type<'src>> {
        match op {
            UnaryOp::Not => Some(Type::Bool),
            UnaryOp::BitNot if Self::is_bitwise_numeric(&inner) => Some(inner),
            UnaryOp::Neg
            | UnaryOp::PreInc
            | UnaryOp::PreDec
            | UnaryOp::PostInc
            | UnaryOp::PostDec
                if Self::numeric_rank(&inner).is_some() =>
            {
                Some(inner)
            }
            _ => None,
        }
    }

    fn infer_binary_type(
        &self,
        op: &BinOp,
        lhs_type: Type<'src>,
        rhs_type: Type<'src>,
    ) -> Option<Type<'src>> {
        match op {
            BinOp::Eq
            | BinOp::NotEq
            | BinOp::Less
            | BinOp::LessEq
            | BinOp::Greater
            | BinOp::GreaterEq
            | BinOp::And
            | BinOp::Or => Some(Type::Bool),

            BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::Mod
            | BinOp::Power
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Shl
            | BinOp::Shr => {
                if self.can_assign(&lhs_type, &rhs_type) {
                    Some(lhs_type)
                } else if self.can_assign(&rhs_type, &lhs_type) {
                    Some(rhs_type)
                } else {
                    None
                }
            }
        }
    }

    fn infer_if_type(
        &self,
        then: &Expr<'src>,
        else_: &Option<Box<Expr<'src>>>,
    ) -> Option<Type<'src>> {
        let then_type = self.infer_expr_type(then)?;
        let else_expr = else_.as_ref()?;
        let else_type = self.infer_expr_type(else_expr)?;

        if then_type == else_type {
            Some(then_type)
        } else {
            None
        }
    }

    // match assignments
    fn infer_assign_type(&self, op: &AssignOp, rhs_type: Type<'src>) -> Option<Type<'src>> {
        match op {
            AssignOp::Assign
            | AssignOp::PlusEq
            | AssignOp::MinusEq
            | AssignOp::StarEq
            | AssignOp::SlashEq
            | AssignOp::PercentEq
            | AssignOp::AndEq
            | AssignOp::OrEq
            | AssignOp::XorEq
            | AssignOp::ShlEq
            | AssignOp::ShrEq => Some(rhs_type),
        }
    }

    // common coersions. (ints -> 64 bits, the rest is obvious)
    fn infer_literal_type(&self, literal: &Literal<'src>) -> Type<'src> {
        match literal {
            Literal::Int(_) => Type::I64,
            Literal::Uint(_) => Type::U64,
            Literal::Float(_) => Type::F32,
            Literal::Double(_) => Type::F64,
            Literal::Bool(_) => Type::Bool,
            Literal::Char(_) => Type::Char,
            Literal::String(_) => Type::Str,
            Literal::Unit => Type::Unit,
        }
    }

    fn infer_expr_type(&self, expr: &Expr<'src>) -> Option<Type<'src>> {
        match expr {
            Expr::Literal(lit) => Some(self.infer_literal_type(lit)),

            Expr::Ident(ident) => self.types.get(ident.0).cloned(),

            Expr::Unary { op, expr } => {
                let inner = self.infer_expr_type(expr)?;
                self.infer_unary_type(op, inner)
            }

            Expr::Binary { op, lhs, rhs } => {
                let lhs_type = self.infer_expr_type(lhs)?;
                let rhs_type = self.infer_expr_type(rhs)?;

                self.infer_binary_type(op, lhs_type, rhs_type)
            }

            Expr::Assign { op, lhs: _, rhs } => {
                let rhs_type = self.infer_expr_type(rhs)?;

                self.infer_assign_type(op, rhs_type)
            }

            Expr::Block { tail, .. } => tail.as_ref().and_then(|t| self.infer_expr_type(t)),

            Expr::If { then, else_, .. } => self.infer_if_type(then, else_),

            _ => None,
        }
    }

    // in declarations, check for the following:
    fn check_decl(&mut self, node: &Stmt<'src>) {
        // first form it into a usable vardecl
        let Stmt::VarDecl {
            name,
            typ,
            init,
            mutable: _,
            constant: _,
            global: _,
        } = node
        else {
            return;
        };

        // this code pattern is gross and i need to fix it but i had to go to class
        let init_type = init.as_ref().and_then(|value| self.infer_expr_type(value));
        let resolved_type = match (typ, init_type.as_ref()) {
            (Type::Inferred, Some(actual)) => actual.clone(),
            (Type::Inferred, None) => Type::Error,
            (declared, Some(actual)) if self.can_assign(declared, actual) => declared.clone(),
            (_, Some(_)) => Type::Error,
            (declared, None) => declared.clone(),
        };

        if resolved_type == Type::Error {
            return;
        }

        if !self.resolved.contains_key(name.0) {
            let next_id = self.resolved.len();
            self.resolved.insert(name.0, next_id);
        }

        self.types.insert(name.0, resolved_type);
    }

    pub fn analyze(&mut self) {
        while let Some(node) = self.cur().cloned() {
            if matches!(node, Stmt::VarDecl { .. }) {
                self.check_decl(&node);
            }

            self.pos += 1;
        }

        // based on calls check certain things
        // hint: match statement

        // branch for variable declarations

        // branch for variable reassignments

        // branch for binary ops

        // branch for
    }
}
