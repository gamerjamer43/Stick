use std::ops::Range;
use std::{collections::HashMap, time::Instant};

use crate::{
    error::{Diagnostic, SemanticError, SemanticError::*, SyntaxError, SyntaxError::*},
    parser::ast::{AssignOp, BinOp, Expr, Ident, LeftSide, Literal, Stmt, Type, UnaryOp},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConstFit {
    Fits,
    Overflow,
    Incompatible,
}

#[derive(Debug, Clone)]
struct Binding<'src> {
    id: usize,
    typ: Type<'src>,
    mutable: bool,
    value: Option<ConstValue>,
}

pub struct Analyzer<'a, 'src> {
    pub path: &'a str,
    pub src: &'src str,
    pub nodes: Vec<Stmt<'src>>,
    pub resolved: HashMap<&'src str, usize>,
    pub types: HashMap<&'src str, Type<'src>>,
    pub errors: Vec<Diagnostic<'a, 'src>>,

    scopes: Vec<HashMap<&'src str, Binding<'src>>>,
    functions: HashMap<&'src str, Type<'src>>,
    next_symbol_id: usize,
}

impl<'a, 'src> Analyzer<'a, 'src> {
    pub fn new(path: &'a str, src: &'src str, nodes: Vec<Stmt<'src>>) -> Self {
        let functions = Self::collect_functions(&nodes);
        Self {
            path,
            src,
            nodes,
            resolved: HashMap::new(),
            types: HashMap::new(),
            errors: Vec::new(),
            scopes: vec![HashMap::new()],
            functions,
            next_symbol_id: 0,
        }
    }

    pub fn symbol_count(&self) -> usize {
        self.next_symbol_id
    }

    fn collect_functions(nodes: &[Stmt<'src>]) -> HashMap<&'src str, Type<'src>> {
        nodes.iter()
            .filter_map(|stmt| match stmt {
                Stmt::FuncDecl {
                    name, typ, args, ..
                } => Some((
                    name.0,
                    Type::Func {
                        params: args.iter().map(|(_, typ)| typ.clone()).collect(),
                        ret: Box::new(typ.clone()),
                    },
                )),
                _ => None,
            })
            .collect()
    }

    fn push_error(&mut self, span: Range<usize>, err: SyntaxError<'src>) {
        self.errors.push(Diagnostic {
            path: self.path,
            src: self.src,
            span,
            err,
        });
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn current_scope_mut(&mut self) -> &mut HashMap<&'src str, Binding<'src>> {
        self.scopes
            .last_mut()
            .expect("analyzer must always keep at least one scope")
    }

    fn sync_exports(&mut self) {
        self.resolved.clear();
        self.types.clear();

        if let Some(root) = self.scopes.first() {
            for (&name, binding) in root {
                self.resolved.insert(name, binding.id);
                self.types.insert(name, binding.typ.clone());
            }
        }
    }

    fn lookup_binding(&self, name: &'src str) -> Option<&Binding<'src>> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn lookup_binding_mut(&mut self, name: &'src str) -> Option<&mut Binding<'src>> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
    }

    fn lookup_type(&self, name: &'src str) -> Option<Type<'src>> {
        self.lookup_binding(name)
            .map(|binding| binding.typ.clone())
            .or_else(|| self.functions.get(name).cloned())
    }

    fn resolve_ident_type(&mut self, ident: &Ident<'src>) -> Option<Type<'src>> {
        self.lookup_type(ident.0).or_else(|| {
            self.push_error(ident.span(), Semantic(UnknownIdentifier(ident.0)));
            None
        })
    }

    fn declare_binding(
        &mut self,
        name: &Ident<'src>,
        typ: Type<'src>,
        mutable: bool,
        value: Option<ConstValue>,
    ) -> bool {
        if self.scopes.last().is_some_and(|scope| scope.contains_key(name.0)) {
            self.push_error(
                name.span(),
                Semantic(InvalidOperation("identifier is already declared in this scope")),
            );
            return false;
        }

        let id = self.next_symbol_id;
        self.next_symbol_id += 1;
        let value = self.sanitized_const_value(&typ, value);

        self.current_scope_mut().insert(
            name.0,
            Binding {
                id,
                typ,
                mutable,
                value,
            },
        );

        true
    }

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

    fn numeric_family(typ: &Type<'src>) -> Option<u8> {
        match typ {
            Type::I8 | Type::I16 | Type::I32 | Type::I64 => Some(0),
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => Some(1),
            Type::F32 | Type::F64 => Some(2),
            _ => None,
        }
    }

    fn is_numeric(typ: &Type<'src>) -> bool {
        Self::numeric_rank(typ).is_some()
    }

    fn is_integral(typ: &Type<'src>) -> bool {
        matches!(
            typ,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
        )
    }

    fn is_signed_numeric(typ: &Type<'src>) -> bool {
        matches!(typ, Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::F32 | Type::F64)
    }

    /// check if two types can be compared for equality
    fn are_comparable_for_equality(lhs: &Type<'src>, rhs: &Type<'src>) -> bool {
        (Self::is_numeric(lhs) && Self::is_numeric(rhs))
            || (lhs == rhs)
    }

    /// check if two types can be compared for ordering
    fn are_comparable_for_ordering(lhs: &Type<'src>, rhs: &Type<'src>) -> bool {
        Self::is_numeric(lhs) && Self::is_numeric(rhs)
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

    fn eval_int_binary(&self, op: BinOp, lhs: i128, rhs: i128) -> Option<ConstValue> {
        use BinOp::*;
        use ConstValue::*;

        Some(match op {
            Add => Int(lhs.checked_add(rhs)?),
            Sub => Int(lhs.checked_sub(rhs)?),
            Mul => Int(lhs.checked_mul(rhs)?),
            Div => Int(lhs.checked_div(rhs)?),
            Mod => Int(lhs.checked_rem(rhs)?),
            Power => Int(lhs.checked_pow(u32::try_from(rhs).ok()?)?),
            Eq => Bool(lhs == rhs),
            NotEq => Bool(lhs != rhs),
            Less => Bool(lhs < rhs),
            LessEq => Bool(lhs <= rhs),
            Greater => Bool(lhs > rhs),
            GreaterEq => Bool(lhs >= rhs),
            BitAnd => Int(lhs & rhs),
            BitOr => Int(lhs | rhs),
            BitXor => Int(lhs ^ rhs),
            Shl => Int(lhs.checked_shl(u32::try_from(rhs).ok()?)?),
            Shr => Int(lhs.checked_shr(u32::try_from(rhs).ok()?)?),
            And | Or => return None,
        })
    }

    fn eval_float_binary(&self, op: BinOp, lhs: f64, rhs: f64) -> Option<ConstValue> {
        use BinOp::*;
        use ConstValue::*;

        Some(match op {
            Add => Float(lhs + rhs),
            Sub => Float(lhs - rhs),
            Mul => Float(lhs * rhs),
            Div if rhs == 0.0 => return None,
            Div => Float(lhs / rhs),
            Mod if rhs == 0.0 => return None,
            Mod => Float(lhs % rhs),
            Power => Float(lhs.powf(rhs)),
            Eq => Bool(lhs == rhs),
            NotEq => Bool(lhs != rhs),
            Less => Bool(lhs < rhs),
            LessEq => Bool(lhs <= rhs),
            Greater => Bool(lhs > rhs),
            GreaterEq => Bool(lhs >= rhs),
            And | Or | BitAnd | BitOr | BitXor | Shl | Shr => return None,
        })
    }

    fn eval_bool_binary(&self, op: BinOp, lhs: bool, rhs: bool) -> Option<ConstValue> {
        use BinOp::*;
        use ConstValue::*;

        Some(match op {
            Eq => Bool(lhs == rhs),
            NotEq => Bool(lhs != rhs),
            And => Bool(lhs && rhs),
            Or => Bool(lhs || rhs),
            _ => return None,
        })
    }

    /// in the case a constant can be folded, it will
    fn eval_const(&self, expr: &Expr<'src>) -> Option<ConstValue> {
        use ConstValue::*;
        use UnaryOp::*;

        match expr {
            Expr::Literal(literal) => self.const_from_literal(literal),
            Expr::Ident(ident) => self.lookup_binding(ident.0).and_then(|binding| binding.value.clone()),

            Expr::Unary { op, expr } => match (op, self.eval_const(expr)?) {
                (Neg, Int(value)) => Some(Int(value.checked_neg()?)),
                (Neg, Float(value)) => Some(Float(-value)),
                (Not, Bool(value)) => Some(Bool(!value)),
                (BitNot, Int(value)) => Some(Int(!value)),
                _ => None,
            },

            Expr::Binary { op, lhs, rhs } => match (self.eval_const(lhs)?, self.eval_const(rhs)?) {
                (Int(lhs), Int(rhs)) => self.eval_int_binary(*op, lhs, rhs),
                (Float(lhs), Float(rhs)) => self.eval_float_binary(*op, lhs, rhs),
                (Bool(lhs), Bool(rhs)) => self.eval_bool_binary(*op, lhs, rhs),
                _ => None,
            },

            Expr::Block { tail, .. } => tail.as_ref().and_then(|expr| self.eval_const(expr)),
            Expr::If { cond, then, else_ } => match self.eval_const(cond)? {
                Bool(true) => self.eval_const(then),
                Bool(false) => else_.as_ref().and_then(|expr| self.eval_const(expr)),
                _ => None,
            },
            _ => None,
        }
    }

    fn const_fit(&self, declared: &Type<'src>, value: &ConstValue) -> ConstFit {
        use ConstFit::*;

        let int_fit = |fits: bool| if fits { Fits } else { Overflow };

        match (declared, value) {
            (Type::I8, ConstValue::Int(value)) => int_fit(i8::try_from(*value).is_ok()),
            (Type::I16, ConstValue::Int(value)) => int_fit(i16::try_from(*value).is_ok()),
            (Type::I32, ConstValue::Int(value)) => int_fit(i32::try_from(*value).is_ok()),
            (Type::I64, ConstValue::Int(value)) => int_fit(i64::try_from(*value).is_ok()),
            (Type::U8, ConstValue::Int(value)) => int_fit(u8::try_from(*value).is_ok()),
            (Type::U16, ConstValue::Int(value)) => int_fit(u16::try_from(*value).is_ok()),
            (Type::U32, ConstValue::Int(value)) => int_fit(u32::try_from(*value).is_ok()),
            (Type::U64, ConstValue::Int(value)) => int_fit(u64::try_from(*value).is_ok()),
            (Type::F32 | Type::F64, ConstValue::Int(_)) => Fits,
            (Type::F32, ConstValue::Float(value)) => {
                int_fit(value.is_finite() && *value >= f32::MIN as f64 && *value <= f32::MAX as f64)
            }
            (Type::F64, ConstValue::Float(value)) => int_fit(value.is_finite()),
            (Type::Bool, ConstValue::Bool(_))
            | (Type::Char, ConstValue::Char)
            | (Type::Str, ConstValue::String)
            | (Type::Unit, ConstValue::Unit) => Fits,
            _ => Incompatible,
        }
    }

    fn sanitized_const_value(&self, typ: &Type<'src>, value: Option<ConstValue>) -> Option<ConstValue> {
        value.filter(|value| self.const_fit(typ, value) == ConstFit::Fits)
    }

    // confirms you can assign a type properly
    fn can_assign(&self, expected: &Type<'src>, actual: &Type<'src>) -> bool {
        if expected == actual {
            return true;
        }

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
        lhs_type: &Type<'src>,
        rhs_type: &Type<'src>,
    ) -> Option<Type<'src>> {
        if self.can_assign(lhs_type, rhs_type) {
            Some(lhs_type.clone())
        } else if self.can_assign(rhs_type, lhs_type) {
            Some(rhs_type.clone())
        } else {
            None
        }
    }

    fn binding_is_mutable(&self, name: &'src str) -> bool {
        self.lookup_binding(name).is_some_and(|binding| binding.mutable)
    }

    fn require_mutable_ident(&mut self, ident: &Ident<'src>) -> bool {
        if self.binding_is_mutable(ident.0) {
            true
        } else {
            self.push_error(ident.span(), Semantic(ImmutableBinding(ident.0)));
            false
        }
    }

    fn resolve_assign_target(&mut self, lhs: &LeftSide<'src>) -> Option<(&'src str, Type<'src>)> {
        match lhs {
            LeftSide::Var(ident) => {
                let typ = self.resolve_ident_type(ident)?;
                if !self.require_mutable_ident(ident) {
                    return None;
                }
                Some((ident.0, typ))
            }

            LeftSide::Field { .. } | LeftSide::Subscript { .. } => {
                self.push_error(
                    lhs.span(),
                    Semantic(InvalidOperation(
                        "assignment analysis currently only supports plain variable targets",
                    )),
                );
                None
            }
        }
    }

    fn check_unary_expr(
        &mut self,
        op: &UnaryOp,
        expr: &Expr<'src>,
        inner: Type<'src>,
    ) -> Option<Type<'src>> {
        let invalid = || Semantic(InvalidOperation("invalid unary operation for operand type"));

        match op {
            UnaryOp::Not if inner == Type::Bool => Some(Type::Bool),
            UnaryOp::BitNot if Self::is_integral(&inner) => Some(inner),
            UnaryOp::Neg if Self::is_signed_numeric(&inner) => Some(inner),
            UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec => {
                let Expr::Ident(ident) = expr else {
                    self.push_error(
                        expr.span(),
                        Semantic(InvalidOperation(
                            "increment and decrement require a mutable variable target",
                        )),
                    );
                    return None;
                };

                if !self.require_mutable_ident(ident) {
                    return None;
                }

                if Self::is_numeric(&inner) {
                    Some(inner)
                } else {
                    self.push_error(expr.span(), invalid());
                    None
                }
            }
            _ => {
                self.push_error(expr.span(), invalid());
                None
            }
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

    fn check_binary_op(
        &self,
        op: &BinOp,
        lhs_type: Type<'src>,
        rhs_type: Type<'src>,
    ) -> Result<Type<'src>, SemanticError<'src>> {
        use BinOp::*;

        match op {
            Add | Sub | Mul | Div | Mod | Power => self
                .common_numeric_type(&lhs_type, &rhs_type)
                .ok_or(InvalidOperation(
                    "binary operands are not type-compatible for this operator",
                )),

            BitAnd | BitOr | BitXor | Shl | Shr
                if Self::is_integral(&lhs_type) && Self::is_integral(&rhs_type) =>
            {
                self.common_numeric_type(&lhs_type, &rhs_type)
                    .ok_or(InvalidOperation(
                        "binary operands are not type-compatible for this operator",
                    ))
            }

            BitAnd | BitOr | BitXor | Shl | Shr => Err(InvalidOperation(
                "bitwise operators require integral operands",
            )),

            And | Or if lhs_type == Type::Bool && rhs_type == Type::Bool => Ok(Type::Bool),
            And | Or => Err(InvalidOperation("logical operators require bool operands")),

            Eq | NotEq if Self::are_comparable_for_equality(&lhs_type, &rhs_type) => Ok(Type::Bool),
            Eq | NotEq => Err(InvalidOperation(
                "equality operands must have the same or compatible numeric types",
            )),

            Less | LessEq | Greater | GreaterEq
                if Self::are_comparable_for_ordering(&lhs_type, &rhs_type) =>
            {
                Ok(Type::Bool)
            }

            Less | LessEq | Greater | GreaterEq => Err(InvalidOperation(
                "comparison operators require compatible numeric operands",
            )),
        }
    }

    fn infer_if_type(
        &mut self,
        cond: &Expr<'src>,
        then: &Expr<'src>,
        else_: Option<&Expr<'src>>,
        expected: Option<&Type<'src>>,
    ) -> Option<Type<'src>> {
        let cond_type = self.infer_expr_type_with_hint(cond, Some(&Type::Bool))?;
        if cond_type != Type::Bool {
            self.push_error(
                cond.span(),
                Semantic(TypeMismatch("if condition must evaluate to bool")),
            );
            return None;
        }

        let then_type = self.infer_expr_type_with_hint(
            then,
            else_.map_or(Some(&Type::Unit), |_| expected),
        )?;

        match else_ {
            Some(else_expr) => {
                let else_type = self.infer_expr_type_with_hint(else_expr, expected)?;

                if then_type == else_type {
                    Some(then_type)
                } else {
                    self.push_error(
                        then.span(),
                        Semantic(TypeMismatch("if branches must evaluate to the same type")),
                    );
                    None
                }
            }
            None if then_type == Type::Unit => Some(Type::Unit),
            None => {
                self.push_error(
                    then.span(),
                    Semantic(TypeMismatch("if without else must evaluate to unit")),
                );
                None
            }
        }
    }

    // ensure all argument types match (and arg count as well)
    fn check_call(&mut self, func: &Expr<'src>, args: &[Expr<'src>]) -> Option<Type<'src>> {
        let callable = self.infer_expr_type(func)?;

        let Type::Func { params, ret } = callable else {
            self.push_error(
                func.span(),
                Semantic(InvalidOperation("attempted to call a non-function value")),
            );
            return None;
        };

        if params.len() != args.len() {
            self.push_error(
                func.span(),
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
                    arg.span(),
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
                if Self::is_numeric(target_type) && self.can_assign(target_type, rhs_type) =>
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
                if Self::is_integral(target_type) && self.can_assign(target_type, rhs_type) =>
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

    fn store_assignment_value(
        &mut self,
        name: &'src str,
        op: &AssignOp,
        typ: &Type<'src>,
        value: Option<ConstValue>,
    ) {
        let value = match op {
            AssignOp::Assign => self.sanitized_const_value(typ, value),
            _ => None,
        };

        if let Some(binding) = self.lookup_binding_mut(name) {
            binding.value = value;
        }
    }

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
        let Some(expected) = expected else {
            return self.infer_literal_type(literal);
        };

        match self.const_from_literal(literal).as_ref().map(|value| self.const_fit(expected, value)) {
            Some(ConstFit::Fits) => expected.clone(),
            _ => self.infer_literal_type(literal),
        }
    }

    fn check_assign_expr(
        &mut self,
        op: &AssignOp,
        lhs: &LeftSide<'src>,
        rhs: &Expr<'src>,
    ) -> Option<Type<'src>> {
        let (name, target_type) = self.resolve_assign_target(lhs)?;
        let inferred_rhs_type = self.infer_expr_type_with_hint(rhs, Some(&target_type))?;
        let folded_rhs = self.eval_const(rhs);
        let rhs_type = match folded_rhs.as_ref().map(|value| self.const_fit(&target_type, value)) {
            Some(ConstFit::Fits) => target_type.clone(),
            _ => inferred_rhs_type,
        };

        match self.check_assign_compatibility(op, &target_type, &rhs_type) {
            Ok(()) => {
                self.store_assignment_value(name, op, &target_type, folded_rhs);
                Some(target_type)
            }

            Err(err) => {
                let err = match folded_rhs.as_ref().map(|value| self.const_fit(&target_type, value)) {
                    Some(ConstFit::Overflow) => Overflow("assigned constant overflows the target type"),
                    _ => err,
                };

                self.push_error(rhs.span(), Semantic(err));
                None
            }
        }
    }

    fn infer_expr_type_with_hint(
        &mut self,
        expr: &Expr<'src>,
        expected: Option<&Type<'src>>,
    ) -> Option<Type<'src>> {
        match expr {
            Expr::Literal(lit) => Some(self.infer_literal_type_with_hint(lit, expected)),
            Expr::Ident(ident) => self.resolve_ident_type(ident),
            Expr::Unary { op, expr } => {
                let inner = self.infer_expr_type_with_hint(expr, expected)?;
                self.check_unary_expr(op, expr, inner)
            }
            Expr::Binary { op, lhs, rhs } => {
                let lhs_type = self.infer_expr_type_with_hint(lhs, expected)?;
                let rhs_type = self.infer_expr_type_with_hint(rhs, expected)?;

                match self.check_binary_op(op, lhs_type, rhs_type) {
                    Ok(typ) => Some(typ),
                    Err(err) => {
                        self.push_error(lhs.span(), Semantic(err));
                        None
                    }
                }
            }
            Expr::Call { func, args } => self.check_call(func, args),
            Expr::Assign { op, lhs, rhs } => self.check_assign_expr(op, lhs, rhs),
            Expr::Block { stmts, tail } => self.analyze_block_expr(stmts, tail.as_deref(), expected),
            Expr::If { cond, then, else_ } => {
                self.infer_if_type(cond, then, else_.as_deref(), expected)
            }
            _ => None,
        }
    }

    fn infer_expr_type(&mut self, expr: &Expr<'src>) -> Option<Type<'src>> {
        self.infer_expr_type_with_hint(expr, None)
    }

    fn function_type_from_parts(&self, params: &[(Ident<'src>, Type<'src>)], ret: &Type<'src>) -> Type<'src> {
        Type::Func {
            params: params.iter().map(|(_, typ)| typ.clone()).collect(),
            ret: Box::new(ret.clone()),
        }
    }

    fn check_decl(&mut self, node: &Stmt<'src>) {
        let Stmt::VarDecl {
            name,
            typ,
            init,
            mutable,
            ..
        } = node
        else {
            return;
        };

        let folded_init = init.as_ref().and_then(|expr| self.eval_const(expr));
        let init_type = match (typ, init.as_ref(), folded_init.as_ref()) {
            (declared, Some(_), Some(value))
                if !matches!(declared, Type::Inferred)
                    && self.const_fit(declared, value) == ConstFit::Fits =>
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
                name.span(),
                Semantic(TypeInference(
                    "could not infer a valid type from initializer",
                )),
            );
            return;
        }

        let resolved_type = match self.resolve_decl_type(typ, init_type.as_ref()) {
            Ok(resolved) => resolved,
            Err(err) => {
                let err = match folded_init.as_ref().map(|value| self.const_fit(typ, value)) {
                    Some(ConstFit::Overflow) => Overflow("initializer constant overflows the declared type"),
                    _ => err,
                };

                self.push_error(name.span(), Semantic(err));
                return;
            }
        };

        self.declare_binding(name, resolved_type, *mutable, folded_init);
    }

    fn check_func_decl(&mut self, node: &Stmt<'src>) {
        let Stmt::FuncDecl {
            name, typ, args, ..
        } = node
        else {
            return;
        };

        let func_type = self.function_type_from_parts(args, typ);
        self.declare_binding(name, func_type, false, None);
    }

    fn analyze_stmt(&mut self, stmt: &Stmt<'src>) {
        match stmt {
            Stmt::Expr(expr) => {
                let _ = self.infer_expr_type(expr);
            }
            Stmt::Return(Some(expr)) => {
                let _ = self.infer_expr_type(expr);
            }
            Stmt::VarDecl { .. } => self.check_decl(stmt),
            Stmt::FuncDecl { .. } => self.check_func_decl(stmt),

            // unchecked for right now (break and continue should only be used in loops)
            // error shouldn't exist at this stage
            Stmt::Return(None) | Stmt::Break | Stmt::Continue | Stmt::Error | Stmt::Include { .. } => {}
        }
    }

    // analyze every statement inside of a block
    fn analyze_block_expr(
        &mut self,
        stmts: &[Stmt<'src>],
        tail: Option<&Expr<'src>>,
        expected: Option<&Type<'src>>,
    ) -> Option<Type<'src>> {
        self.enter_scope();

        for stmt in stmts {
            self.analyze_stmt(stmt);
        }

        let tail_type = tail.and_then(|tail_expr| self.infer_expr_type_with_hint(tail_expr, expected));
        self.exit_scope();
        tail_type
    }

    pub fn analyze(&mut self) {
        let start = Instant::now();

        for idx in 0..self.nodes.len() {
            let node = self.nodes[idx].clone();
            self.analyze_stmt(&node);
        }

        self.sync_exports();

        println!(
            "Analyzed {} symbols in {}s.",
            self.symbol_count(),
            start.elapsed().as_secs_f64()
        );

        if !self.errors.is_empty() {
            for error in &self.errors {
                eprintln!("{error}");
            }
            eprintln!("\n(!) {} semantic errors found.", self.errors.len());
        }
    }
}
