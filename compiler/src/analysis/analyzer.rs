use std::{collections::HashMap, time::Instant};
use std::ops::Range;

use crate::{
    error::{Diagnostic, SemanticError, SemanticError::*, SyntaxError, SyntaxError::*},
    parser::ast::{AssignOp, BinOp, Expr, LeftSide, Literal, Stmt, Type, UnaryOp},
};

#[derive(Debug, Clone)]
/// typed constant values and their container. ints use i128 because i'm not doing an i65
enum ConstValue {
    Int(i128),
    Float(f64),
    Bool(bool),
    Char,
    String,
    Unit,
}

pub struct Analyzer<'a, 'src> {
    pub path: &'a str,
    pub src: &'src str,
    pub nodes: Vec<Stmt<'src>>,
    pub resolved: HashMap<&'src str, usize>, // usize contains a unique symbol id
    pub types: HashMap<&'src str, Type<'src>>,
    pub errors: Vec<Diagnostic<'a, 'src>>,
    pub pos: usize,

    // mutability is now also tracked (cuz i forgot to do that)
    mutability: HashMap<&'src str, bool>,
    values: HashMap<&'src str, ConstValue>,
}

impl<'a, 'src> Analyzer<'a, 'src> {
    pub fn new(
        path: &'a str, 
        src: &'src str, 
        nodes: Vec<Stmt<'src>>
    ) -> Self {
        Self {
            path,
            src,
            nodes,
            resolved: HashMap::new(),
            types: HashMap::new(),
            mutability: HashMap::new(),
            values: HashMap::new(),
            errors: Vec::new(),
            pos: 0,
        }
    }

    fn push_error(&mut self, span: Range<usize>, err: SyntaxError<'src>) {
        self.errors.push(Diagnostic {
            path: self.path,
            src: self.src,
            span,
            err,
        });
    }

    // span helpers for errors
    fn span_for_ident(&self, ident: &crate::parser::ast::Ident<'src>) -> Range<usize> {
        ident.span()
    }

    fn span_for_literal(&self, literal: &Literal<'src>) -> Range<usize> {
        literal.span()
    }

    fn span_for_expr(&self, expr: &Expr<'src>) -> Range<usize> {
        match expr {
            Expr::Ident(ident) => self.span_for_ident(ident),
            Expr::Literal(literal) => self.span_for_literal(literal),
            Expr::Unary { expr, .. } => self.span_for_expr(expr),
            Expr::Binary { lhs, .. } => self.span_for_expr(lhs),
            Expr::Assign { rhs, .. } => self.span_for_expr(rhs),
            Expr::If { cond, .. } => self.span_for_expr(cond),
            Expr::Block { tail, .. } => tail
                .as_ref()
                .map(|expr| self.span_for_expr(expr))
                .unwrap_or(0..0),
            _ => 0..0,
        }
    }

    // properly span lhs on error (this was buggin b4)
    fn span_for_left_side(&self, lhs: &LeftSide<'src>) -> Range<usize> {
        match lhs {
            LeftSide::Var(ident) => self.span_for_ident(ident),
            LeftSide::Field { name, .. } => self.span_for_ident(name),
            LeftSide::Subscript { obj, .. } => self.span_for_expr(obj),
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

    // they also have a family, need to figure out more coersion related shit
    fn numeric_family(typ: &Type<'src>) -> Option<u8> {
        match typ {
            Type::I8 | Type::I16 | Type::I32 | Type::I64 => Some(0),
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => Some(1),
            Type::F32 | Type::F64 => Some(2),
            _ => None,
        }
    }

    fn const_from_literal(&self, literal: &Literal<'src>) -> Option<ConstValue> {
        match literal {
            Literal::Int(value, _) => Some(ConstValue::Int(value.replace('_', "").parse().ok()?)),
            Literal::Float(value, _) => {
                Some(ConstValue::Float(value.replace('_', "").parse().ok()?))
            }
            Literal::Bool(value, _) => Some(ConstValue::Bool(*value)),
            Literal::Char(_, _) => Some(ConstValue::Char),
            Literal::String(_, _) => Some(ConstValue::String),
            Literal::Unit(_) => Some(ConstValue::Unit),
        }
    }

    /// in the case a constant can be folded, it will
    fn eval_const(&self, expr: &Expr<'src>) -> Option<ConstValue> {
        use BinOp::*;
        use ConstValue::*;
        use UnaryOp::*;

        match expr {
            Expr::Literal(literal) => self.const_from_literal(literal),
            Expr::Ident(ident) => self.values.get(ident.0).cloned(),

            Expr::Unary { op, expr } => match (op, self.eval_const(expr)?) {
                (Neg, Int(value)) => Some(Int(-value)),
                (Neg, Float(value)) => Some(Float(-value)),
                (Not, Bool(value)) => Some(Bool(!value)),
                (BitNot, Int(value)) => Some(Int(!value)),
                _ => None,
            },

            Expr::Binary { op, lhs, rhs } => {
                let lhs = self.eval_const(lhs)?;
                let rhs = self.eval_const(rhs)?;

                match (op, lhs, rhs) {
                    (Add, Int(a), Int(b)) => Some(Int(a + b)),
                    (Sub, Int(a), Int(b)) => Some(Int(a - b)),
                    (Mul, Int(a), Int(b)) => Some(Int(a * b)),

                    // div and mod by zero return none
                    (Div, Int(_), Int(0)) | (Mod, Int(_), Int(0)) => None,
                    (Div, Int(a), Int(b)) => Some(Int(a / b)),
                    (Mod, Int(a), Int(b)) => Some(Int(a % b)),

                    (Add, Float(a), Float(b)) => Some(Float(a + b)),
                    (Sub, Float(a), Float(b)) => Some(Float(a - b)),
                    (Mul, Float(a), Float(b)) => Some(Float(a * b)),

                    // div and mod by zero return none
                    (Div, Float(_), Float(0.0)) => None,
                    (Mod, Float(_), Float(0.0)) => None,
                    (Div, Float(a), Float(b)) => Some(Float(a / b)),
                    (Mod, Float(a), Float(b)) => Some(Float(a % b)),

                    (Eq, Int(a), Int(b)) => Some(Bool(a == b)),
                    (NotEq, Int(a), Int(b)) => Some(Bool(a != b)),
                    (Less, Int(a), Int(b)) => Some(Bool(a < b)),
                    (LessEq, Int(a), Int(b)) => Some(Bool(a <= b)),
                    (Greater, Int(a), Int(b)) => Some(Bool(a > b)),
                    (GreaterEq, Int(a), Int(b)) => Some(Bool(a >= b)),

                    (Eq, Float(a), Float(b)) => Some(Bool(a == b)),
                    (NotEq, Float(a), Float(b)) => Some(Bool(a != b)),
                    (Less, Float(a), Float(b)) => Some(Bool(a < b)),
                    (LessEq, Float(a), Float(b)) => Some(Bool(a <= b)),
                    (Greater, Float(a), Float(b)) => Some(Bool(a > b)),
                    (GreaterEq, Float(a), Float(b)) => Some(Bool(a >= b)),

                    (Eq, Bool(a), Bool(b)) => Some(Bool(a == b)),
                    (NotEq, Bool(a), Bool(b)) => Some(Bool(a != b)),
                    (And, Bool(a), Bool(b)) => Some(Bool(a && b)),
                    (Or, Bool(a), Bool(b)) => Some(Bool(a || b)),
                    _ => None,
                }
            }

            // for blocks: check tail (TODO: below is a stubbed check_block)
            Expr::Block { tail, .. } => tail.as_ref().and_then(|expr| self.eval_const(expr)),

            // check conditions
            Expr::If { cond, then, else_ } => match self.eval_const(cond)? {
                Bool(true) => self.eval_const(then),
                Bool(false) => else_.as_ref().and_then(|expr| self.eval_const(expr)),
                _ => None,
            },
            _ => None,
        }
    }

    fn can_coerce_const_to_declared(&self, declared: &Type<'src>, value: &ConstValue) -> bool {
        match (declared, value) {
            (Type::I8, ConstValue::Int(value)) => i8::try_from(*value).is_ok(),
            (Type::I16, ConstValue::Int(value)) => i16::try_from(*value).is_ok(),
            (Type::I32, ConstValue::Int(value)) => i32::try_from(*value).is_ok(),
            (Type::I64, ConstValue::Int(value)) => i64::try_from(*value).is_ok(),
            (Type::U8, ConstValue::Int(value)) => u8::try_from(*value).is_ok(),
            (Type::U16, ConstValue::Int(value)) => u16::try_from(*value).is_ok(),
            (Type::U32, ConstValue::Int(value)) => u32::try_from(*value).is_ok(),
            (Type::U64, ConstValue::Int(value)) => u64::try_from(*value).is_ok(),
            (Type::F32 | Type::F64, ConstValue::Int(_)) => true,
            (Type::F32 | Type::F64, ConstValue::Float(value)) => {
                !self.const_overflows_declared(declared, &ConstValue::Float(*value))
            }
            (Type::Bool, ConstValue::Bool(_)) => true,
            (Type::Char, ConstValue::Char) => true,
            (Type::Str, ConstValue::String) => true,
            (Type::Unit, ConstValue::Unit) => true,
            _ => false,
        }
    }

    fn const_overflows_declared(&self, declared: &Type<'src>, value: &ConstValue) -> bool {
        match (declared, value) {
            (Type::I8, ConstValue::Int(value)) => i8::try_from(*value).is_err(),
            (Type::I16, ConstValue::Int(value)) => i16::try_from(*value).is_err(),
            (Type::I32, ConstValue::Int(value)) => i32::try_from(*value).is_err(),
            (Type::I64, ConstValue::Int(value)) => i64::try_from(*value).is_err(),
            (Type::U8, ConstValue::Int(value)) => u8::try_from(*value).is_err(),
            (Type::U16, ConstValue::Int(value)) => u16::try_from(*value).is_err(),
            (Type::U32, ConstValue::Int(value)) => u32::try_from(*value).is_err(),
            (Type::U64, ConstValue::Int(value)) => u64::try_from(*value).is_err(),
            (Type::F32, ConstValue::Float(value)) => {
                !value.is_finite() || *value < f32::MIN as f64 || *value > f32::MAX as f64
            }
            (Type::F64, ConstValue::Float(value)) => !value.is_finite(),
            _ => false,
        }
    }

    fn store_const_value(&mut self, name: &'src str, typ: &Type<'src>, value: Option<ConstValue>) {
        if value
            .as_ref()
            .is_some_and(|value| self.can_coerce_const_to_declared(typ, value))
        {
            self.values.insert(name, value.unwrap());
        } else {
            self.values.remove(name);
        }
    }

    // confirms you can assign a type properly
    fn can_assign(&self, expected: &Type<'src>, actual: &Type<'src>) -> bool {
        if expected == actual {
            return true;
        }

        // idk how to make this pattern cleaner
        match (
            Self::numeric_family(expected),
            Self::numeric_family(actual),
            Self::numeric_rank(expected),
            Self::numeric_rank(actual),
        ) {
            (
                Some(expected_family),
                Some(actual_family),
                Some(expected_rank),
                Some(actual_rank),
            ) => expected_family == actual_family && expected_rank >= actual_rank,

            _ => false,
        }
    }

    fn common_numeric_type(
        &self,
        lhs_type: Type<'src>,
        rhs_type: Type<'src>,
    ) -> Option<Type<'src>> {
        if self.can_assign(&lhs_type, &rhs_type) {
            Some(lhs_type)
        } else if self.can_assign(&rhs_type, &lhs_type) {
            Some(rhs_type)
        } else {
            None
        }
    }

    // if type can be given a rank (and isnt a float) we can do bitwise ops
    fn is_bitwise_numeric(typ: &Type<'src>) -> bool {
        match Self::numeric_rank(typ) {
            Some(_) => !matches!(typ, Type::F32 | Type::F64),
            None => false,
        }
    }

    fn resolve_assign_target(&mut self, lhs: &LeftSide<'src>) -> Option<(&'src str, Type<'src>)> {
        match lhs {
            LeftSide::Var(ident) => {
                let typ = match self.types.get(ident.0).cloned() {
                    Some(typ) => typ,
                    None => {
                        self.push_error(
                            self.span_for_ident(ident),
                            Semantic(UnknownIdentifier(ident.0)),
                        );
                        return None;
                    }
                };

                if !self.binding_is_mutable(ident.0) {
                    self.push_error(
                        self.span_for_ident(ident),
                        Semantic(ImmutableBinding(ident.0)),
                    );
                    return None;
                }

                Some((ident.0, typ))
            }

            LeftSide::Field { .. } | LeftSide::Subscript { .. } => {
                self.push_error(
                    self.span_for_left_side(lhs),
                    Semantic(InvalidOperation(
                        "assignment analysis currently only supports plain variable targets",
                    )),
                );
                None
            }
        }
    }

    fn binding_is_mutable(&self, name: &'src str) -> bool {
        self.mutability.get(name).copied().unwrap_or(false)
    }

    fn require_mutable_ident(&mut self, ident: &crate::parser::ast::Ident<'src>) -> bool {
        if !self.binding_is_mutable(ident.0) {
            self.push_error(
                self.span_for_ident(ident),
                Semantic(ImmutableBinding(ident.0)),
            );
            return false;
        }

        true
    }

    fn check_unary_mutation(&mut self, op: &UnaryOp, expr: &Expr<'src>) -> bool {
        if !matches!(
            op,
            UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec
        ) {
            return true;
        }

        match expr {
            Expr::Ident(ident) => self.require_mutable_ident(ident),
            _ => {
                self.push_error(
                    self.span_for_expr(expr),
                    Semantic(InvalidOperation(
                        "increment and decrement require a mutable variable target",
                    )),
                );
                false
            }
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

    fn resolve_decl_type(
        &self,
        declared: &Type<'src>,
        init_type: Option<&Type<'src>>,
    ) -> Result<Type<'src>, SemanticError<'src>> {
        match (declared, init_type) {
            (Type::Inferred, Some(actual)) => Ok(actual.clone()),
            (Type::Inferred, None) => Err(TypeInference(
                "inferred declarations require an initializer",
            )),

            (declared, None) => Ok(declared.clone()),
            (declared, Some(actual)) if self.can_assign(declared, actual) => Ok(declared.clone()),

            _ => Err(TypeMismatch(
                "initializer type is not assignable to declared type",
            )),
        }
    }

    #[inline]
    fn infer_binary_type(
        &self,
        op: &BinOp,
        lhs_type: Type<'src>,
        rhs_type: Type<'src>,
    ) -> Option<Type<'src>> {
        if op.is_comparison_or_logical() {
            Some(Type::Bool)
        } else if op.is_arithmetic() || op.is_bitwise() {
            self.common_numeric_type(lhs_type, rhs_type)
        } else {
            None
        }
    }

    #[inline]
    fn infer_if_type(
        &mut self,
        then: &Expr<'src>,
        else_: &Option<Box<Expr<'src>>>,
        expected: Option<&Type<'src>>,
    ) -> Option<Type<'src>> {
        let then_type = self.infer_expr_type_with_hint(then, expected)?;
        let else_expr = else_.as_ref()?;
        let else_type = self.infer_expr_type_with_hint(else_expr, expected)?;

        if then_type == else_type {
            Some(then_type)
        } else {
            self.push_error(
                self.span_for_expr(then),
                Semantic(TypeMismatch("if branches must evaluate to the same type")),
            );
            None
        }
    }

    // resolve the declared type of a function
    fn declared_func_type(&self, name: &'src str) -> Option<Type<'src>> {
        self.nodes.iter().find_map(|stmt| match stmt {
            Stmt::FuncDecl {
                name: func_name,
                typ,
                args,
                ..
            } if func_name.0 == name => Some(Type::Func {
                params: args.iter().map(|(_, typ)| typ.clone()).collect(),
                ret: Box::new(typ.clone()),
            }),
            _ => None,
        })
    }

    // helps to ensure when a callable is assigned to something, its of the proper type
    fn resolve_callable_type(&mut self, func: &Expr<'src>) -> Option<Type<'src>> {
        match func {
            Expr::Ident(ident) => {
                if let Some(found) = self.types.get(ident.0).cloned() {
                    Some(found)
                } else if let Some(found) = self.declared_func_type(ident.0) {
                    Some(found)
                } else {
                    self.push_error(
                        self.span_for_ident(ident),
                        Semantic(UnknownIdentifier(ident.0)),
                    );
                    None
                }
            }
            _ => self.infer_expr_type(func),
        }
    }

    // ensure all argument types match (and arg count as well)
    fn check_call(&mut self, func: &Expr<'src>, args: &[Expr<'src>]) -> Option<Type<'src>> {
        let callable = self.resolve_callable_type(func)?;

        let Type::Func { params, ret } = callable else {
            self.push_error(
                self.span_for_expr(func),
                Semantic(InvalidOperation("attempted to call a non-function value")),
            );
            return None;
        };

        if params.len() != args.len() {
            self.push_error(
                self.span_for_expr(func),
                Semantic(InvalidOperation(
                    "function call argument count does not match parameter count",
                )),
            );
            return None;
        }

        for (arg, param) in args.iter().zip(params.iter()) {
            let actual = self.infer_expr_type_with_hint(arg, Some(param))?;
            let assignable = matches!(param, Type::Inferred) || self.can_assign(param, &actual);

            if !assignable {
                self.push_error(
                    self.span_for_expr(arg),
                    Semantic(TypeMismatch(
                        "function argument type is not assignable to parameter type",
                    )),
                );
                return None;
            }
        }

        Some(*ret)
    }

    // ensure a value can be properly assigned to a type
    fn check_assign_compatibility(
        &self,
        op: &AssignOp,
        target_type: &Type<'src>,
        rhs_type: &Type<'src>,
    ) -> Result<(), SemanticError<'src>> {
        match op {
            AssignOp::Assign if self.can_assign(target_type, rhs_type) => Ok(()),
            AssignOp::Assign => Err(TypeMismatch(
                "assigned value is not assignable to the target type",
            )),

            AssignOp::PlusEq
            | AssignOp::MinusEq
            | AssignOp::StarEq
            | AssignOp::SlashEq
            | AssignOp::PercentEq
                if Self::numeric_rank(target_type).is_some()
                    && self.can_assign(target_type, rhs_type) =>
            {
                Ok(())
            }

            AssignOp::PlusEq
            | AssignOp::MinusEq
            | AssignOp::StarEq
            | AssignOp::SlashEq
            | AssignOp::PercentEq => Err(InvalidOperation(
                "compound arithmetic assignment requires a numeric target and compatible value",
            )),

            AssignOp::AndEq
            | AssignOp::OrEq
            | AssignOp::XorEq
            | AssignOp::ShlEq
            | AssignOp::ShrEq
                if Self::is_bitwise_numeric(target_type)
                    && self.can_assign(target_type, rhs_type) =>
            {
                Ok(())
            }

            AssignOp::AndEq
            | AssignOp::OrEq
            | AssignOp::XorEq
            | AssignOp::ShlEq
            | AssignOp::ShrEq => Err(InvalidOperation(
                "bitwise assignment requires an integral target and compatible value",
            )),
        }
    }

    // store the updated value in the value pool (store_const_value is poorly named)
    fn store_assignment_value(
        &mut self,
        name: &'src str,
        op: &AssignOp,
        typ: &Type<'src>,
        value: Option<ConstValue>,
    ) {
        match op {
            AssignOp::Assign => self.store_const_value(name, typ, value),
            _ => {
                self.values.remove(name);
            }
        }
    }

    // make sure assignments can be done (mutability and type check)
    fn check_assign_expr(
        &mut self,
        op: &AssignOp,
        lhs: &LeftSide<'src>,
        rhs: &Expr<'src>,
    ) -> Option<Type<'src>> {
        let (name, target_type) = self.resolve_assign_target(lhs)?;
        let inferred_rhs_type = self.infer_expr_type_with_hint(rhs, Some(&target_type))?;
        let folded_rhs = self.eval_const(rhs);
        let rhs_type = if folded_rhs
            .as_ref()
            .is_some_and(|value| self.can_coerce_const_to_declared(&target_type, value))
        {
            target_type.clone()
        } else {
            inferred_rhs_type
        };

        match self.check_assign_compatibility(op, &target_type, &rhs_type) {
            Ok(()) => {
                self.store_assignment_value(name, op, &target_type, folded_rhs);
                Some(target_type)
            }

            Err(err) => {
                let err = if folded_rhs
                    .as_ref()
                    .is_some_and(|value| self.const_overflows_declared(&target_type, value))
                {
                    Overflow("assigned constant overflows the target type")
                } else {
                    err
                };

                self.push_error(self.span_for_expr(rhs), Semantic(err));
                None
            }
        }
    }

    // ensure a literal can be coerced to the type its being reassigned to
    fn can_coerce_literal_to_declared(
        &self,
        declared: &Type<'src>,
        literal: &Literal<'src>,
    ) -> bool {
        self.const_from_literal(literal)
            .as_ref()
            .is_some_and(|value| self.can_coerce_const_to_declared(declared, value))
    }

    // defaults for inferred literals: int => i64, float => f64, unsigned => u64
    fn infer_literal_type(&self, literal: &Literal<'src>) -> Type<'src> {
        match literal {
            Literal::Int(_, _) => Type::I64,
            Literal::Float(_, _) => Type::F64,
            Literal::Bool(_, _) => Type::Bool,
            Literal::Char(_, _) => Type::Char,
            Literal::String(_, _) => Type::Str,
            Literal::Unit(_) => Type::Unit,
        }
    }

    fn infer_literal_type_with_hint(
        &self,
        literal: &Literal<'src>,
        expected: Option<&Type<'src>>,
    ) -> Type<'src> {
        match expected {
            Some(expected) if self.can_coerce_literal_to_declared(expected, literal) => {
                expected.clone()
            }
            _ => self.infer_literal_type(literal),
        }
    }

    fn infer_expr_type_with_hint(
        &mut self,
        expr: &Expr<'src>,
        expected: Option<&Type<'src>>,
    ) -> Option<Type<'src>> {
        match expr {
            Expr::Literal(lit) => Some(self.infer_literal_type_with_hint(lit, expected)),

            Expr::Ident(ident) => {
                if let Some(found) = self.types.get(ident.0).cloned() {
                    Some(found)
                } else {
                    self.push_error(
                        self.span_for_ident(ident),
                        Semantic(UnknownIdentifier(ident.0)),
                    );
                    None
                }
            }

            Expr::Unary { op, expr } => {
                let inner = self.infer_expr_type_with_hint(expr, expected)?;
                if !self.check_unary_mutation(op, expr) {
                    return None;
                }
                match self.infer_unary_type(op, inner) {
                    Some(typ) => Some(typ),
                    None => {
                        self.push_error(
                            self.span_for_expr(expr),
                            Semantic(InvalidOperation(
                                "invalid unary operation for inferred operand type",
                            )),
                        );
                        None
                    }
                }
            }

            Expr::Binary { op, lhs, rhs } => {
                let lhs_type = self.infer_expr_type_with_hint(lhs, expected)?;
                let rhs_type = self.infer_expr_type_with_hint(rhs, expected)?;

                if let Some(expected) = expected {
                    let matches_expected = if op.is_arithmetic() {
                        Self::numeric_rank(expected).is_some()
                            && self.can_assign(expected, &lhs_type)
                            && self.can_assign(expected, &rhs_type)
                    } else if op.is_bitwise() {
                        Self::is_bitwise_numeric(expected)
                            && self.can_assign(expected, &lhs_type)
                            && self.can_assign(expected, &rhs_type)
                    } else if op.is_comparison_or_logical() {
                        matches!(expected, Type::Bool)
                    } else {
                        false
                    };

                    if matches_expected {
                        return Some(expected.clone());
                    }
                }

                match self.infer_binary_type(op, lhs_type, rhs_type) {
                    Some(typ) => Some(typ),
                    None => {
                        self.push_error(
                            self.span_for_expr(lhs),
                            Semantic(InvalidOperation(
                                "binary operands are not type-compatible for this operator",
                            )),
                        );
                        None
                    }
                }
            }

            Expr::Call { func, args } => self.check_call(func, args),

            Expr::Assign { op, lhs, rhs } => self.check_assign_expr(op, lhs, rhs),

            Expr::Block { stmts, tail } => {
                self.analyze_block_expr(stmts, tail.as_deref(), expected)
            }

            Expr::If { then, else_, .. } => self.infer_if_type(then, else_, expected),

            _ => None,
        }
    }

    fn infer_expr_type(&mut self, expr: &Expr<'src>) -> Option<Type<'src>> {
        self.infer_expr_type_with_hint(expr, None)
    }

    // analyze every statement inside of a block
    fn analyze_block_expr(
        &mut self,
        stmts: &[Stmt<'src>],
        tail: Option<&Expr<'src>>,
        expected: Option<&Type<'src>>,
    ) -> Option<Type<'src>> {
        for stmt in stmts {
            match stmt {
                Stmt::VarDecl { .. } => self.check_decl(stmt),
                Stmt::Expr(expr) => {
                    let _ = self.infer_expr_type(expr);
                }
                Stmt::Return(Some(expr)) => {
                    let _ = self.infer_expr_type(expr);
                }
                _ => {}
            }
        }

        tail.and_then(|tail_expr| self.infer_expr_type_with_hint(tail_expr, expected))
    }

    // in declarations, check for the following:
    fn check_decl(&mut self, node: &Stmt<'src>) {
        // first form it into a usable vardecl
        let Stmt::VarDecl {
            name,
            typ,
            init,
            mutable,
            constant: _,
            global: _,
        } = node
        else {
            return;
        };

        let folded_init = init.as_ref().and_then(|expr| self.eval_const(expr));
        let init_type = match (typ, init.as_ref(), folded_init.as_ref()) {
            (declared, Some(_), Some(value))
                if !matches!(declared, Type::Inferred)
                    && self.can_coerce_const_to_declared(declared, value) =>
            {
                Some(declared.clone())
            }

            (declared, Some(value), _) if !matches!(declared, Type::Inferred) => {
                self.infer_expr_type_with_hint(value, Some(declared))
            }

            (_, Some(value), _) => self.infer_expr_type(value),
            (_, None, _) => None,
        };

        if init.is_some() && init_type.is_none() {
            self.push_error(
                self.span_for_ident(name),
                Semantic(TypeInference(
                    "could not infer a valid type from initializer",
                )),
            );
            return;
        }

        let resolved_type = match self.resolve_decl_type(typ, init_type.as_ref()) {
            Ok(resolved) => resolved,
            Err(err) => {
                self.push_error(self.span_for_ident(name), Semantic(err));
                return;
            }
        };

        if !self.resolved.contains_key(name.0) {
            let next_id = self.resolved.len();
            self.resolved.insert(name.0, next_id);
        }

        self.store_const_value(name.0, &resolved_type, folded_init);
        self.types.insert(name.0, resolved_type);
        self.mutability.insert(name.0, *mutable);
    }

    pub fn analyze(&mut self) {
        let start: Instant = Instant::now();
        
        while let Some(node) = self.cur().cloned() {
            match node {
                Stmt::Expr(expr) => {
                    let _ = self.infer_expr_type(&expr);
                }
                // Stmt::Return( .. ) => self.check_return(&node),
                // Stmt::Break => self.check_break(&node),
                // Stmt::Continue => self.check_continue(&node),
                // Stmt::FuncDecl { .. } => self.check_func(&node),
                // Stmt::Error => self.check_stmt_error(&node),
                
                Stmt::VarDecl { .. } => self.check_decl(&node),

                _ => {}
            }

            self.pos += 1;
        }

        println!(
            "Analyzed {} symbols in {}s.",
            self.resolved.len(), 
            start.elapsed().as_secs_f64()
        );

        if !self.errors.is_empty() {
            for error in &self.errors {
                eprintln!("{error}");
            }
            println!("\n(!) {} semantic errors found.", self.errors.len());
        }
    }
}
