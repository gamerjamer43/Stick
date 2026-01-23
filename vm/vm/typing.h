/**
 * @file typing.h
 * @author Noah Mingolelli
 * @brief a header for anything that has to do with assigning type
 * the instruction typedef, type enum, and value struct all live here
 * just to separate this from the general vm header
 *
 * Instruction -> u32 (to use fixed 32 bit packed ints for instructions)
 *
 * Type     -> enum        (to properly extract the packed value, see macros for types)
 * FuncType -> enum        (2 options, NATIVE function from C, or a normal BYTECODE function)
 *
 * TODO: figure out how to deal with bit width besides holding everything as an int
 * Value -> struct, fields:
 * - u8 type;         (you know what this means, the type the Value currently has attatched to it)
 * - u8 val[8];   (the bitwise value of that type, up to 8 bytes. u/i128 will be heap allocated, or stored low/high in 2 registers)
 *
 * Func -> struct, fields:
 * - FuncType kind;        (either BYTECODE for local or NATIVE for C)
 * - union as;             (so you can call the function properly based on its type)
 *   - BytecodeFunc bc;    (for BYTECODE functions)
 *   - NativeFunc nat;     (for NATIVE functions)
 *
 * BytecodeFunc -> struct, fields:
 * - u32 entry_ip;    (instruction index where function starts)
 * - u16 argc;        (number of arguments)
 * - u16 regc;        (number of registers needed)
 *
 * NativeFunc -> struct, fields:
 * - NativeFn fn;          (pointer to the C native function)
 * - u16 argc;        (number of arguments)
 * - u16 _pad;        (padding for alignment)
 */

#ifndef TYPING_H
#define TYPING_H

#include <stdint.h>
#include <stdbool.h>

// various aliases (as i hate how long stdint names r)
typedef uint8_t  u8;
typedef int8_t   i8;
typedef uint16_t u16;
typedef int16_t  i16;
typedef uint32_t u32;
typedef int32_t  i32;
typedef uint64_t u64;
typedef int64_t  i64;

// instructions are 32 bit packed. (opcode << 24 | src0 << 16 | src1 << 8 | src2 << 0)
typedef u32 Instruction;

// fields, used both for extracting instructions, and the byte arrays attached to Value.val
typedef u8 Field;

// type listing up here (maybe enum idk)
enum Type {
    NUL      = 0,   // standard "None"/"null" type
    BOOL     = 1,   // a true or false value (represented by 0 or 1)
    U64      = 2,   // an unsigned 64 bit integer
    I64      = 3,   // a signed 64 bit integer
    FLOAT    = 4,   // a 32 bit single precision float
    DOUBLE   = 5,   // a 64 bit double precision float
    OBJ      = 6,   // a general object
    CALLABLE = 7,   // a callable
};

// these macros allow for me to expect a condition to be more likely true or false
// still a compiler hint but will liekly happen
#if defined(__GNUC__) || defined(__clang__)
  #define LIKELYTRUE(x)   __builtin_expect(!!(x), 1)
  #define LIKELYFALSE(x) __builtin_expect(!!(x), 0)
#else
  #define LIKELYTRUE(x)   (x)
  #define LIKELYFALSE(x) (x)
#endif

// starting amount of registers for entry frame
#define BASE_REGISTERS 16

// max amount of registers and frames we can grow to
#define MAX_REGISTERS 65536
#define MAX_FRAMES 256

// typed operations/type conversions (ugly but it works and is cycle light)
// type requirement will be removed in prod once i add a verifier
// macro helpers for typed arithmetic and comparisons
#define BINOP_TYPED(TAG, FIELD, OP) do { \
    u32 dest, lhs, rhs; \
    if (!binop_indices(vm, ins, &dest, &lhs, &rhs)) return false; \
    if (!require_type(vm, lhs, (TAG)) || !require_type(vm, rhs, (TAG))) return false; \
    vm->regs->types[dest] = (TAG); \
    vm->regs->payloads[dest].FIELD = vm->regs->payloads[lhs].FIELD OP vm->regs->payloads[rhs].FIELD; \
} while (0)

#define CMPOP_TYPED(TAG, FIELD, OP) do { \
    u32 dest, lhs, rhs; \
    if (!binop_indices(vm, ins, &dest, &lhs, &rhs)) return false; \
    if (!require_type(vm, lhs, (TAG)) || !require_type(vm, rhs, (TAG))) return false; \
    vm->regs->types[dest] = BOOL; \
    vm->regs->payloads[dest].u = (vm->regs->payloads[lhs].FIELD OP vm->regs->payloads[rhs].FIELD) ? 1u : 0u; \
} while (0)

#define UNOP_TYPED(TAG, FIELD, OP) do { \
    u32 idx; \
    if (!unary_index(vm, ins, &idx)) return false; \
    if (!require_type(vm, idx, (TAG))) return false; \
    vm->regs->types[idx] = (TAG); \
    vm->regs->payloads[idx].FIELD = OP vm->regs->payloads[idx].FIELD; \
} while (0)

// conversion helper (dest = op_a, src = op_b)
#define CAST_TYPED(SRC_TAG, SRC_FIELD, DST_TAG, DST_FIELD, EXPR) do { \
    u32 dest = op_a(ins) + vm->current->base; \
    u32 src  = op_b(ins) + vm->current->base; \
    u32 need = (dest > src ? dest : src) + 1; \
    if (!ensure_regs(vm, need)) return false; \
    if (!require_type(vm, src, (SRC_TAG))) return false; \
    vm->regs->types[dest] = (DST_TAG); \
    vm->regs->payloads[dest].DST_FIELD = (EXPR); \
} while (0)

// aliases for typed ops to remove some clutter
#define BINOP_I64(OP) BINOP_TYPED(I64, i, OP)
#define BINOP_U64(OP) BINOP_TYPED(U64, u, OP)
#define BINOP_F32(OP) BINOP_TYPED(FLOAT, f, OP)
#define BINOP_F64(OP) BINOP_TYPED(DOUBLE, d, OP)

#define CMPOP_I64(OP) CMPOP_TYPED(I64, i, OP)
#define CMPOP_U64(OP) CMPOP_TYPED(U64, u, OP)
#define CMPOP_F32(OP) CMPOP_TYPED(FLOAT, f, OP)
#define CMPOP_F64(OP) CMPOP_TYPED(DOUBLE, d, OP)

#define UNOP_I64(OP) UNOP_TYPED(I64, i, OP)
#define UNOP_U64(OP) UNOP_TYPED(U64, u, OP)
#define UNOP_F32(OP) UNOP_TYPED(FLOAT, f, OP)
#define UNOP_F64(OP) UNOP_TYPED(DOUBLE, d, OP)


// support for native C functions will be added.
// this is so webservers and shit can exist
typedef enum {
    BYTECODE,
    NATIVE,
} FuncType;

// gotta forward decl to use in TypedValue
typedef struct Func Func;

// forward decl so NativeFn can take VM*, and this is also used in heap.h
typedef struct VM VM;

// standardized 9 byte value struct, problem with padding solved due to u8 packing
// problem with overhead solved due to register file
typedef struct {
    u8 type;
    u8 val[8];
} Value;

// the actual payload holding the value inside of a register
typedef union {
    i64    i;
    u64    u;
    double d;
    float  f;
    bool   b;
    Func*  fn;

    // change to an obj pointer shits fucked rn
    void*  obj;
} TypedValue;

// this is either gonna be a pointer to a val, or a payload containing a value. width is canonical
// so you can use u8 but extensions r basically noop.
typedef struct {
    TypedValue payloads[MAX_REGISTERS];
    u8 types[MAX_REGISTERS];
} Registers;

// allow C natives (pointer to a function that takes these args, this is a feature of the language)
// to be properly passed to value. base is the register index where args start
typedef void (*NativeFn)(VM* vm, u32 base, u16 argc, u32 dest);

// function types
typedef struct {
    u32 entry_ip;  // instruction index

    // TODO: decide if this is 16 bit or 8 bit. instructions only rly allow for 8 bit
    u16 argc;      // how many args this takes
    u16 regc;      // how many registers this call needs when it runs
} BytecodeFunc;

typedef struct {
    NativeFn fn;  // pointer to the C native function
    u16  argc;     // how many args this takes
    u16  _pad;     // keep alignment nice
} NativeFunc;

// properly resolve both bytecode and native functions as callables
typedef struct Func {
    FuncType kind;
    union {
        BytecodeFunc bc;
        NativeFunc   nat;
    } as;
} Func;

#endif