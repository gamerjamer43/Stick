"""
simple writer for VM tests
writes a 20 byte header + u32 instruction stream,
plus optional const/global pools (raw Value bytes).
little endian.
"""
from dataclasses import dataclass
from enum import IntEnum, auto
from struct import pack
from pathlib import Path

# verbose flag
VERBOSE: bool = False

# header (may change), version and flags
HEADER: bytes = b"STIK"
VERSION: int = 1
FLAGS: int = 1 if VERBOSE else 0

# opcode table (see vm/opcodes.h)
class Opcode(IntEnum):
    HALT = 0; PANIC = auto()

    # ip movement
    JMP = auto(); JMPIF = auto(); JMPIFZ = auto()

    # register movement
    COPY = auto(); MOVE = auto(); LOADI = auto(); LOADC = auto(); LOADG = auto(); STOREG = auto()

    # call stack
    CALL = auto(); TAILCALL = auto(); RET = auto()

    # bitwise
    AND = auto(); OR = auto(); XOR = auto()
    LNOT = auto(); BNOT = auto()
    SHL = auto(); SHR = auto();

    # heap/tables/arrays/strings (placeholders for now)
    NEWARR = auto(); NEWTABLE = auto(); NEWOBJ = auto()
    GETELEM = auto(); SETELEM = auto()
    ARRGET = auto(); ARRSET = auto(); ARRLEN = auto()
    CONCAT = auto(); STRLEN = auto(); NEWSTR = auto()

    # conversions
    I2D = auto(); I2F = auto(); D2I = auto(); F2I = auto(); I2U = auto(); U2I = auto()
    U2D = auto(); U2F = auto(); D2U = auto(); F2U = auto()

    # arithmetic (signed i64)
    ADD = auto(); SUB = auto(); MUL = auto(); DIV = auto(); MOD = auto(); NEG = auto()

    # comparisons (signed i64)
    EQ = auto(); NEQ = auto(); GT = auto(); GE = auto(); LT = auto(); LE = auto()

    # typed unsigned 64-bit
    ADD_U = auto(); SUB_U = auto(); MUL_U = auto(); DIV_U = auto(); MOD_U = auto(); NEG_U = auto()
    EQ_U = auto(); NEQ_U = auto(); GT_U = auto(); GE_U = auto(); LT_U = auto(); LE_U = auto()

    # typed float32
    ADD_F = auto(); SUB_F = auto(); MUL_F = auto(); DIV_F = auto(); NEG_F = auto()
    EQ_F = auto(); NEQ_F = auto(); GT_F = auto(); GE_F = auto(); LT_F = auto(); LE_F = auto()

    # typed float64
    ADD_D = auto(); SUB_D = auto(); MUL_D = auto(); DIV_D = auto(); NEG_D = auto()
    EQ_D = auto(); NEQ_D = auto(); GT_D = auto(); GE_D = auto(); LT_D = auto(); LE_D = auto()

    # typed bitwise (unsigned)
    AND_U = auto(); OR_U = auto(); XOR_U = auto(); SHL_U = auto(); SHR_U = auto(); BNOT_U = auto()

# type tags (typing.h)
class Type(IntEnum):
    NUL = 0
    BOOL = auto()
    U64 = auto(); I64 = auto()
    FLOAT = auto(); DOUBLE = auto()
    OBJ = auto()
    CALLABLE = auto()

# type helpers (pack to 9 bytes each)
def nul() -> bytes:            return pack("<Bq", Type.NUL, 0)
def boolean(v: bool) -> bytes: return pack("<Bq", Type.BOOL, 1 if v else 0)
def u64(v: int) -> bytes:      return pack("<BQ", Type.U64, v)
def i64(v: int) -> bytes:      return pack("<Bq", Type.I64, v)
def f32(v: float) -> bytes:    return pack("<Bf", Type.FLOAT, v) + b"\x00" * 4
def f64(v: float) -> bytes:    return pack("<Bd", Type.DOUBLE, v)
def func(entry: int, argc: int, regc: int) -> bytes:
    """4 byte function entry, 2 byte argc, 2 byte regc"""
    return pack("<BIHH", Type.CALLABLE, entry, argc, regc)

# ported directly from c lmao
def ins(op, a = 0, b = 0, c = 0) -> int:
    return (int(op) << 24) | ((a & 0xFF) << 16) | ((b & 0xFF) << 8) | (c & 0xFF)

# general instruction helpers
def HALT():             return ins(Opcode.HALT)
def PANIC(code=1):      return ins(Opcode.PANIC, code)
def LOADI(r, n):        return ins(Opcode.LOADI, r, (n >> 8) & 0xFF, n & 0xFF)
def LOADC(r, idx):      return ins(Opcode.LOADC, r, idx)
def LOADG(r, idx):      return ins(Opcode.LOADG, r, idx)
def STOREG(r, idx):     return ins(Opcode.STOREG, r, idx)
def COPY(dst, src):     return ins(Opcode.COPY, dst, src)
def MOVE(dst, src):     return ins(Opcode.MOVE, dst, src)
def JMP(off):           return ins(Opcode.JMP, (off >> 16) & 0xFF, (off >> 8) & 0xFF, off & 0xFF)
def JMPIF(r, off):      return ins(Opcode.JMPIF, r, 0, off)
def JMPIFZ(r, off):     return ins(Opcode.JMPIFZ, r, 0, off)
def BIN(op, dst, a, b): return ins(op, dst, a, b)
def UN(op, r):          return ins(op, r)

# heap / array / string helpers
def NEWARR(dst, elem_type, cap_reg): return ins(Opcode.NEWARR, dst, int(elem_type), cap_reg)
def ARRGET(dst, arr, idx):           return ins(Opcode.ARRGET, dst, arr, idx)
def ARRSET(arr, idx, val):           return ins(Opcode.ARRSET, arr, idx, val)
def ARRLEN(dst, arr):                return ins(Opcode.ARRLEN, dst, arr)
def STRLEN_OP(dst, src):             return ins(Opcode.STRLEN, dst, src)
def CONCAT_OP(dst, a, b):           return ins(Opcode.CONCAT, dst, a, b)

def NEWSTR_WORDS(dest, s: bytes) -> list:
    """pack NEWSTR instruction + inline data words for a raw byte string"""
    length = len(s)
    padded = s + b'\x00' * ((4 - length % 4) % 4)
    words = [ins(Opcode.NEWSTR, dest, (length >> 8) & 0xFF, length & 0xFF)]
    for i in range(0, len(padded), 4):
        words.append(int.from_bytes(padded[i:i+4], 'little'))
    return words

# test model
@dataclass(frozen=True)
class TestCase:
    tag: Opcode
    name: str
    words: list[int]
    consts: tuple[bytes, ...] = ()
    globs: tuple[bytes, ...] = ()

def pass_if_truthy(tag, name, setup, check_reg, consts=(), globs=()):
    """common test for if a reg is NOT zero"""
    words = [*setup, JMPIF(check_reg, 1), PANIC(), HALT()]
    return TestCase(tag, name, words, tuple(consts), tuple(globs))

def pass_if_zero(tag, name, setup, check_reg, consts=(), globs=()):
    """common test for if a reg is zero"""
    words = [*setup, JMPIFZ(check_reg, 1), PANIC(), HALT()]
    return TestCase(tag, name, words, tuple(consts), tuple(globs))

# general tests
TESTS: list[TestCase] = [
    TestCase(Opcode.HALT, "halt", [HALT()]),
    # TestCase(Opcode.PANIC, "panic_code_1", [PANIC(1)]),
    TestCase(Opcode.JMP, "basic_jmp", [JMP(1), PANIC(), HALT()]),
    TestCase(Opcode.JMPIF, "jmpif_taken", [LOADI(0, 1), JMPIF(0, 1), PANIC(), HALT()]),
    TestCase(Opcode.JMPIFZ, "jmpifz_taken", [LOADI(0, 0), JMPIFZ(0, 1), PANIC(), HALT()]),
]

# register movement
TESTS += [
    # check both registers to confirm the copy
    TestCase(Opcode.COPY, "copy_nonzero", [
        LOADC(0, 0), COPY(1, 0),
        JMPIFZ(1, 2), JMPIFZ(0, 1), HALT(), PANIC()
    ], consts=(i64(7),)),

    # check both registers to confirm the move
    TestCase(Opcode.MOVE, "move_nonzero", [
        LOADC(0, 0), MOVE(1, 0),
        JMPIF(0, 2), JMPIFZ(1, 1), HALT(), PANIC()
    ], consts=(i64(7),)),
]

# load/store (imms, consts, globals)
TESTS += [
    pass_if_truthy(Opcode.LOADI, "loadi_nonzero", [LOADI(0, 5)], 0),
    pass_if_zero(Opcode.LOADC, "loadc_zero", [LOADC(0, 1)], 0, consts=(i64(5), i64(0))),
    pass_if_truthy(Opcode.LOADG, "loadg_storeg",
        [LOADC(0, 0), STOREG(0, 0), LOADG(1, 0)], 1,
        consts=(i64(42),), globs=(i64(0),)),
]

# casts
TESTS += [
    pass_if_truthy(Opcode.I2D, "i2d_basic", [
        LOADC(0, 0), BIN(Opcode.I2D, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ_D, 3, 1, 2)
    ], 3, consts=(i64(7), f64(7.0))),

    pass_if_truthy(Opcode.I2F, "i2f_basic", [
        LOADC(0, 0), BIN(Opcode.I2F, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ_F, 3, 1, 2)
    ], 3, consts=(i64(11), f32(11.0))),

    # i2f should round at the f32 mantissa limit (2 to the 24)
    pass_if_truthy(Opcode.I2F, "bonus_i2f_rounding", [
        LOADC(0, 0), BIN(Opcode.I2F, 1, 0, 0),
        LOADC(2, 1), BIN(Opcode.EQ_F, 3, 1, 2)
    ], 3, consts=(i64(16777217), f32(16777216.0))),

    pass_if_truthy(Opcode.D2I, "d2i_basic", [
        LOADC(0, 0), BIN(Opcode.D2I, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ, 3, 1, 2)
    ], 3, consts=(f64(42.0), i64(42))),

    # d2i should always truncate to 0
    pass_if_truthy(Opcode.D2I, "bonus_d2i_trunc", [
        LOADC(0, 0), BIN(Opcode.D2I, 1, 0, 0),
        LOADC(2, 1), BIN(Opcode.EQ, 3, 1, 2)
    ], 3, consts=(f64(-3.7), i64(-3))),

    pass_if_truthy(Opcode.F2I, "f2i_basic", [
        LOADC(0, 0), BIN(Opcode.F2I, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ, 3, 1, 2)
    ], 3, consts=(f32(10.0), i64(10))),

    pass_if_truthy(Opcode.I2U, "i2u_basic", [
        LOADC(0, 0), BIN(Opcode.I2U, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ_U, 3, 1, 2)
    ], 3, consts=(i64(123456789), u64(123456789))),

    pass_if_truthy(Opcode.U2I, "u2i_basic", [
        LOADC(0, 0), BIN(Opcode.U2I, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ, 3, 1, 2)
    ], 3, consts=(u64(55), i64(55))),

    pass_if_truthy(Opcode.U2D, "u2d_basic", [
        LOADC(0, 0), BIN(Opcode.U2D, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ_D, 3, 1, 2)
    ], 3, consts=(u64(9001), f64(9001.0))),

    pass_if_truthy(Opcode.U2F, "u2f_basic", [
        LOADC(0, 0), BIN(Opcode.U2F, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ_F, 3, 1, 2)
    ], 3, consts=(u64(77), f32(77.0))),

    pass_if_truthy(Opcode.D2U, "d2u_basic", [
        LOADC(0, 0), BIN(Opcode.D2U, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ_U, 3, 1, 2)
    ], 3, consts=(f64(123.0), u64(123))),

    pass_if_truthy(Opcode.F2U, "f2u_basic", [
        LOADC(0, 0), BIN(Opcode.F2U, 1, 0, 0), LOADC(2, 1), BIN(Opcode.EQ_U, 3, 1, 2)
    ], 3, consts=(f32(15.0), u64(15))),
]

# arithmetic ops (just check for non zero values)
# load a into r[0] and b into r[1], store op result in r[2]
for op, name, a, b in [
    (Opcode.ADD, "add_basic", 3, 4),
    (Opcode.SUB, "sub_basic", 10, 3),
    (Opcode.MUL, "mul_basic", 3, 4),
    (Opcode.DIV, "div_basic", 20, 4),
    (Opcode.MOD, "mod_basic", 17, 5),
]: TESTS.append(pass_if_truthy(op, name, 
                [LOADI(0, a), LOADI(1, b), 
                 BIN(op, 2, 0, 1)], 2))

# quick unary negation. negate a value, add it to itself, and check if it equals zero
TESTS.append(pass_if_zero(Opcode.NEG, "neg_basic", 
    [LOADI(0, 5), COPY(1, 0), UN(Opcode.NEG, 1), BIN(Opcode.ADD, 2, 0, 1)], 2))

# boolean comparisons
TESTS += [
    pass_if_truthy(Opcode.EQ,  "eq_true",  [LOADI(0, 5), LOADI(1, 5), BIN(Opcode.EQ, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.NEQ, "neq_true", [LOADI(0, 5), LOADI(1, 3), BIN(Opcode.NEQ, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.GT,  "gt_true",  [LOADI(0, 10), LOADI(1, 5), BIN(Opcode.GT, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.LT,  "lt_true",  [LOADI(0, 3), LOADI(1, 10), BIN(Opcode.LT, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.GE,  "ge_equal", [LOADI(0, 5), LOADI(1, 5), BIN(Opcode.GE, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.LE,  "le_equal", [LOADI(0, 5), LOADI(1, 5), BIN(Opcode.LE, 2, 0, 1)], 2),
]

# bitwise ops
TESTS += [
    pass_if_truthy(Opcode.AND, "and_basic", [LOADI(0, 0b1111), LOADI(1, 0b0101), BIN(Opcode.AND, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.OR,  "or_basic",  [LOADI(0, 0b1010), LOADI(1, 0b0101), BIN(Opcode.OR, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.XOR, "xor_basic", [LOADI(0, 0b1111), LOADI(1, 0b1010), BIN(Opcode.XOR, 2, 0, 1)], 2),
    pass_if_zero(Opcode.LNOT, "lnot_basic", [LOADC(0, 0), UN(Opcode.LNOT, 0)], 0, consts=(boolean(True),)),
    # lnot false should be true (non-zero)
    pass_if_truthy(Opcode.LNOT, "bonus_lnot_false", [LOADC(0, 0), UN(Opcode.LNOT, 0)], 0, consts=(boolean(False),)),
    pass_if_truthy(Opcode.BNOT, "bnot_basic", [LOADI(0, 0), UN(Opcode.BNOT, 0)], 0),
    # bnot of 0 should be -1 (all bits set) for i64
    pass_if_truthy(Opcode.BNOT, "bonus_bnot_neg_one", [
        LOADI(0, 0), UN(Opcode.BNOT, 0), LOADC(1, 0), BIN(Opcode.EQ, 2, 0, 1)
    ], 2, consts=(i64(-1),)),
]

# bit shifts
TESTS += [
    pass_if_truthy(Opcode.SHL, "shl_basic", [LOADI(0, 1), LOADI(1, 4), BIN(Opcode.SHL, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.SHR, "shr_basic", [LOADI(0, 16), LOADI(1, 2), BIN(Opcode.SHR, 2, 0, 1)], 2),
    pass_if_truthy(Opcode.SHR, "shr_sar_basic", [
        LOADC(0, 0), LOADI(1, 2), BIN(Opcode.SHR, 2, 0, 1)
    ], 2, consts=(i64(-64),)),
]

# edge testing
TESTS += [
    # test backward jump
    TestCase(Opcode.JMP, "bonus_jmp_backward", [
        LOADI(0, 3), LOADI(1, 1),
        BIN(Opcode.SUB, 0, 0, 1), JMPIFZ(0, 1),
        ins(Opcode.JMP, 0xFF, 0xFF, 0xFD), HALT()
    ]),

    # double check jmpifz
    TestCase(Opcode.JMPIFZ, "bonus_jmpifz_not_taken", [LOADI(0, 1), JMPIFZ(0, 1), HALT(), PANIC()]),

    # make sure loadi is signed
    TestCase(Opcode.LOADI, "bonus_loadi_negative", [LOADI(0, 0xFFFF), JMPIFZ(0, 1), HALT(), PANIC()]),

    # jmpif should treat non-zeroes (including negative zero) as valid
    TestCase(Opcode.JMPIF, "bonus_jmpif_negative_truthy", [
        LOADI(0, 0xFFFF), JMPIF(0, 1), PANIC(), HALT()
    ]),

    # jmp forward more than one instruction
    TestCase(Opcode.JMP, "bonus_jmp_skip_two", [
        JMP(2), PANIC(), PANIC(), HALT()
    ]),

    # check loadi properly sign extends 0x8000 (-32768)
    pass_if_truthy(Opcode.LOADI, "bonus_loadi_sign_min", [
        LOADI(0, 0x8000), LOADC(1, 0), BIN(Opcode.EQ, 2, 0, 1)
    ], 2, consts=(i64(-32768),)),
]

# arithmetic op edge cases (over/underflow, negative vals, and * 0. should be supported)
TESTS += [
    pass_if_truthy(Opcode.ADD, "bonus_add_large",
        [LOADC(0, 0), LOADC(1, 0), BIN(Opcode.ADD, 2, 0, 1)], 2, consts=(i64(0x7FFFFFFF),)),
    pass_if_truthy(Opcode.ADD, "bonus_add_underflow",
        [LOADC(0, 0), LOADC(1, 0), BIN(Opcode.ADD, 2, 0, 1)], 2, consts=(i64(-0x7FFFFFFF),)),
    pass_if_zero(Opcode.SUB, "bonus_sub_zero_result",
        [LOADI(0, 5), LOADI(1, 5), BIN(Opcode.SUB, 2, 0, 1)], 2),
    pass_if_zero(Opcode.MUL, "bonus_mul_by_zero",
        [LOADI(0, 100), LOADI(1, 0), BIN(Opcode.MUL, 2, 0, 1)], 2),
]

# boolean comp edge cases
TESTS += [
    pass_if_zero(Opcode.EQ, "bonus_eq_false", [LOADI(0, 5), LOADI(1, 3), BIN(Opcode.EQ, 2, 0, 1)], 2),
    pass_if_zero(Opcode.GT, "bonus_gt_false_equal", [LOADI(0, 5), LOADI(1, 5), BIN(Opcode.GT, 2, 0, 1)], 2),
    pass_if_zero(Opcode.GT, "bonus_gt_false_less", [LOADI(0, 3), LOADI(1, 5), BIN(Opcode.GT, 2, 0, 1)], 2),
]

# bitwise edge cases (known behavior really should be fine)
TESTS += [
    pass_if_zero(Opcode.AND, "bonus_and_zero", [LOADI(0, 0b1010), LOADI(1, 0b0101), BIN(Opcode.AND, 2, 0, 1)], 2),
    pass_if_zero(Opcode.XOR, "bonus_xor_self_zero", [LOADI(0, 42), BIN(Opcode.XOR, 1, 0, 0)], 1),
]

# typed unsigned, float, and double ops (new b/c i just wrote something that worked not something optimized)
TESTS += [
    pass_if_truthy(Opcode.ADD_U, "add_u_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.ADD_U, 2, 0, 1)
    ], 2, consts=(u64(3), u64(4))),

    pass_if_truthy(Opcode.DIV_U, "div_u_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.DIV_U, 2, 0, 1)
    ], 2, consts=(u64(12), u64(3))),

    pass_if_truthy(Opcode.MOD_U, "mod_u_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.MOD_U, 2, 0, 1)
    ], 2, consts=(u64(13), u64(5))),

    pass_if_truthy(Opcode.EQ_U, "eq_u_true", [
        LOADC(0, 0), LOADC(1, 0), BIN(Opcode.EQ_U, 2, 0, 1)
    ], 2, consts=(u64(123),)),

    pass_if_truthy(Opcode.GT_U, "gt_u_true", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.GT_U, 2, 0, 1)
    ], 2, consts=(u64(10), u64(2))),

    # lil bonus: unsigned comparison should treat the higher bit values as larger than small numbers
    pass_if_truthy(Opcode.GT_U, "bonus_gt_u_highbit", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.GT_U, 2, 0, 1)
    ], 2, consts=(u64(0x8000000000000000), u64(1))),

    pass_if_truthy(Opcode.LE_U, "le_u_true", [
        LOADC(0, 0), LOADC(1, 0), BIN(Opcode.LE_U, 2, 0, 1)
    ], 2, consts=(u64(77),)),

    pass_if_truthy(Opcode.SHL_U, "shl_u_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.SHL_U, 2, 0, 1)
    ], 2, consts=(u64(1), u64(4))),

    pass_if_truthy(Opcode.SHR_U, "shr_u_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.SHR_U, 2, 0, 1)
    ], 2, consts=(u64(16), u64(2))),

    pass_if_truthy(Opcode.AND_U, "and_u_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.AND_U, 2, 0, 1)
    ], 2, consts=(u64(0b1100), u64(0b0101))),

    pass_if_zero(Opcode.XOR_U, "xor_u_zero", [
        LOADC(0, 0), BIN(Opcode.XOR_U, 1, 0, 0)
    ], 1, consts=(u64(0xDEADBEEFCAFEBABE),)),

    pass_if_truthy(Opcode.OR_U, "or_u_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.OR_U, 2, 0, 1)
    ], 2, consts=(u64(0b1000), u64(0b0001))),

    pass_if_truthy(Opcode.BNOT_U, "bnot_u_nonzero", [
        LOADC(0, 0), UN(Opcode.BNOT_U, 0)
    ], 0, consts=(u64(0),)),

    pass_if_truthy(Opcode.ADD_F, "add_f_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.ADD_F, 2, 0, 1)
    ], 2, consts=(f32(1.5), f32(2.5))),

    pass_if_truthy(Opcode.EQ_F, "eq_f_true", [
        LOADC(0, 0), LOADC(1, 0), BIN(Opcode.EQ_F, 2, 0, 1)
    ], 2, consts=(f32(3.25),)),

    pass_if_truthy(Opcode.GT_F, "gt_f_true", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.GT_F, 2, 0, 1)
    ], 2, consts=(f32(5.0), f32(4.0))),

    pass_if_truthy(Opcode.LE_F, "le_f_true", [
        LOADC(0, 0), LOADC(1, 0), BIN(Opcode.LE_F, 2, 0, 1)
    ], 2, consts=(f32(2.0),)),

    pass_if_truthy(Opcode.NEG_F, "neg_f_matches_const", [
        LOADC(0, 0), UN(Opcode.NEG_F, 0), LOADC(1, 1), BIN(Opcode.EQ_F, 2, 0, 1)
    ], 2, consts=(f32(5.5), f32(-5.5))),

    pass_if_truthy(Opcode.ADD_D, "add_d_basic", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.ADD_D, 2, 0, 1)
    ], 2, consts=(f64(1.25), f64(4.75))),

    pass_if_truthy(Opcode.EQ_D, "eq_d_true", [
        LOADC(0, 0), LOADC(1, 0), BIN(Opcode.EQ_D, 2, 0, 1)
    ], 2, consts=(f64(6.5),)),

    pass_if_truthy(Opcode.GT_D, "gt_d_true", [
        LOADC(0, 0), LOADC(1, 1), BIN(Opcode.GT_D, 2, 0, 1)
    ], 2, consts=(f64(9.0), f64(1.0))),

    pass_if_truthy(Opcode.LE_D, "le_d_true", [
        LOADC(0, 0), LOADC(1, 0), BIN(Opcode.LE_D, 2, 0, 1)
    ], 2, consts=(f64(3.0),)),

    pass_if_truthy(Opcode.NEG_D, "neg_d_matches_const", [
        LOADC(0, 0), UN(Opcode.NEG_D, 0), LOADC(1, 1), BIN(Opcode.EQ_D, 2, 0, 1)
    ], 2, consts=(f64(7.75), f64(-7.75))),
]

# test globals with more than one slot
TESTS.append(pass_if_truthy(Opcode.STOREG, "bonus_globals_multi", [
    LOADC(0, 0), LOADC(1, 1),
    STOREG(0, 0), STOREG(1, 1),
    LOADG(2, 0), LOADG(3, 1),
    BIN(Opcode.ADD, 4, 2, 3)
], 4, consts=(i64(10), i64(20)), globs=(i64(0), i64(0))))

# storeg should overwrite existing value at the same index
TESTS.append(pass_if_truthy(Opcode.STOREG, "bonus_globals_overwrite", [
    LOADC(0, 0), STOREG(0, 0),
    LOADC(1, 1), STOREG(1, 0),
    LOADG(2, 0), LOADC(3, 1), BIN(Opcode.EQ, 4, 2, 3)
], 4, consts=(i64(1), i64(2)), globs=(i64(0),)))

# chained arithmetic ops
TESTS.append(pass_if_truthy(Opcode.ADD, "bonus_chained_arith", [
    LOADI(0, 3), LOADI(1, 4), BIN(Opcode.ADD, 2, 0, 1),
    LOADI(3, 2), BIN(Opcode.MUL, 4, 2, 3),
    LOADI(5, 1), BIN(Opcode.SUB, 6, 4, 5)
], 6))

# edge test for double negation
TESTS.append(pass_if_truthy(Opcode.NEG, "bonus_neg_double",
    [LOADI(0, 42), UN(Opcode.NEG, 0), UN(Opcode.NEG, 0)], 0))


# NEW: call and return. getting better at this
TESTS += [
    # function returns 42, caller checks it
    TestCase(Opcode.CALL, "call_ret_basic", [
        LOADC(0, 0), ins(Opcode.CALL, 0, 0, 1),
        JMPIFZ(1, 1), HALT(), PANIC(),
        LOADI(0, 42), ins(Opcode.RET, 0)
    ], consts=(func(5, 0, 4),)),

    # "frame isolation," write is made to r0, but it should remain unchanged 
    # cuz thats where the function pointer lives. call twice, if r0 corrupts the second call fails
    TestCase(Opcode.CALL, "call_frame_isolation", [
        LOADC(0, 0),
        ins(Opcode.CALL, 0, 0, 1), ins(Opcode.CALL, 0, 0, 2),
        JMPIFZ(1, 2), JMPIFZ(2, 1), HALT(), PANIC(),
        LOADI(0, 999), ins(Opcode.RET, 0)
    ], consts=(func(7, 0, 4),)),

    # local vs global, global modified, so check both local and global
    TestCase(Opcode.CALL, "call_local_vs_global", [
        LOADI(1, 100), STOREG(1, 0), LOADC(0, 0),
        ins(Opcode.CALL, 0, 0, 2),
        LOADG(3, 1), JMPIFZ(3, 2),
        LOADG(4, 0), JMPIFZ(4, 1),
        HALT(), PANIC(),
        LOADI(0, 999), STOREG(0, 1), ins(Opcode.RET, 0)
    ], consts=(func(10, 0, 4),), globs=(i64(0), i64(0))),

    # test a tail call (reuses current stack frame)
    # main calls func A, A tailcalls func B, which then returns 123 to main
    # ts was buggin so comments!!!
    TestCase(Opcode.TAILCALL, "call_tailcall_basic", [
        LOADC(0, 0), ins(Opcode.CALL, 0, 0, 1),   # call func A
        JMPIFZ(1, 1), HALT(), PANIC(),            # check result
        LOADC(0, 1), ins(Opcode.TAILCALL, 0, 0),  # tailcall B
        LOADI(0, 123), ins(Opcode.RET, 0)         # func B returns 123
    ], consts=(func(5, 0, 4), func(7, 0, 4))),

    # bytecode CALL should copy args into the callee register window
    TestCase(Opcode.CALL, "call_arg_copy", [
        LOADC(0, 0), LOADC(1, 1), ins(Opcode.CALL, 0, 1, 2),
        LOADC(3, 1), BIN(Opcode.EQ, 4, 2, 3), JMPIFZ(4, 1), HALT(), PANIC(),
        ins(Opcode.RET, 0)
    ], consts=(func(8, 1, 2), i64(42))),
]


# array operations
# create an array of I64s, use arrset to set index 0 to 42, 
# check by using arrget and comparing equality
TESTS.append(pass_if_truthy(Opcode.ARRGET, "arrget_basic", [
    LOADI(0, 4),                             # r0 = 4 (capacity)
    NEWARR(1, Type.I64, 0),                  # r1 = new array[I64](cap=r0)
    LOADI(2, 42),                            # r2 = 42
    LOADI(3, 0),                             # r3 = 0 (index)
    ARRSET(1, 3, 2),                         # arr[0] = 42
    ARRGET(4, 1, 3),                         # r4 = arr[0]
    BIN(Opcode.EQ, 5, 4, 2),                 # r5 = (r4 == r2)
], 5))

# try to get an i64 at a non-zero (but valid) index using arrget
TESTS.append(pass_if_truthy(Opcode.ARRGET, "arrget_index1", [
    LOADI(0, 4),
    NEWARR(1, Type.I64, 0),
    LOADI(2, 10), LOADI(3, 0), ARRSET(1, 3, 2),   # arr[0] = 10
    LOADI(2, 20), LOADI(3, 1), ARRSET(1, 3, 2),   # arr[1] = 20
    LOADI(3, 1), ARRGET(4, 1, 3),                 # r4 = arr[1]
    LOADI(5, 20), BIN(Opcode.EQ, 6, 4, 5),        # r6 = (r4 == 20)
], 6))

# set a value using arrset and read it back (uses truthiness which may be removed)
TESTS.append(pass_if_truthy(Opcode.ARRSET, "arrset_basic", [
    LOADI(0, 4),
    NEWARR(1, Type.I64, 0),
    LOADI(2, 99),
    LOADI(3, 0),
    ARRSET(1, 3, 2),                          # arr[0] = 99
    ARRGET(4, 1, 3),                          # r4 = arr[0]
], 4))

# write multiple elements using arrset
TESTS.append(pass_if_truthy(Opcode.ARRSET, "arrset_multi", [
    LOADI(0, 8),
    NEWARR(1, Type.I64, 0),
    LOADI(2, 7),  LOADI(3, 0), ARRSET(1, 3, 2),   # arr[0] = 7
    LOADI(2, 13), LOADI(3, 1), ARRSET(1, 3, 2),   # arr[1] = 13
    LOADI(2, 21), LOADI(3, 2), ARRSET(1, 3, 2),   # arr[2] = 21
    LOADI(3, 2), ARRGET(4, 1, 3),                 # r4 = arr[2]
    LOADI(5, 21), BIN(Opcode.EQ, 6, 4, 5),        # r6 = (r4 == 21)
], 6))

# set 3 elements, ensure len = 3 with arrlen
TESTS.append(pass_if_truthy(Opcode.ARRLEN, "arrlen_basic", [
    LOADI(0, 8),                             # r0 = 8 (capacity)
    NEWARR(1, Type.I64, 0),                  # arr
    LOADI(2, 10),                            # value
    LOADI(3, 0), ARRSET(1, 3, 2),            # arr[0] = 10
    LOADI(3, 1), ARRSET(1, 3, 2),            # arr[1] = 10
    LOADI(3, 2), ARRSET(1, 3, 2),            # arr[2] = 10
    ARRLEN(4, 1),                            # r4 = length(arr) = 3
    LOADC(5, 0),                             # r5 = u64(3)
    BIN(Opcode.EQ_U, 6, 4, 5),               # r6 = (r4 == r5)
], 6, consts=(u64(3),)))

# arrlen should become index + 1 after setting a high index (resizes)
TESTS.append(pass_if_truthy(Opcode.ARRLEN, "bonus_arrlen_gapped_set", [
    LOADI(0, 8), NEWARR(1, Type.I64, 0),
    LOADI(2, 5), LOADI(3, 4), ARRSET(1, 3, 2),  # arr[4] = 5
    ARRLEN(4, 1), LOADC(5, 0), BIN(Opcode.EQ_U, 6, 4, 5)
], 6, consts=(u64(5),)))

# double check that empty array has length 0
TESTS.append(pass_if_zero(Opcode.ARRLEN, "arrlen_empty", [
    LOADI(0, 4),
    NEWARR(1, Type.I64, 0),
    ARRLEN(2, 1),                            # r2 = length(arr) = 0
], 2))


# string operations
# create "hello" and verify len = 5 with newstr and strlen
TESTS.append(pass_if_truthy(Opcode.NEWSTR, "newstr_basic", [
    *NEWSTR_WORDS(0, b"hello"),              # r0 = "hello"
    STRLEN_OP(1, 0),                         # r1 = strlen(r0) = 5
    LOADC(2, 0),                             # r2 = u64(5)
    BIN(Opcode.EQ_U, 3, 1, 2),               # r3 = (r1 == r2)
], 3, consts=(u64(5),)))

# shouldn't ignore any nulls inside the string when doing length (cuz they arent the null term)
TESTS.append(pass_if_truthy(Opcode.STRLEN, "bonus_newstr_embedded_null", [
    *NEWSTR_WORDS(0, b"a\x00b"),
    STRLEN_OP(1, 0), LOADC(2, 0), BIN(Opcode.EQ_U, 3, 1, 2)
], 3, consts=(u64(3),)))

# test newstr with exactly 4 bytes to check padding logic
TESTS.append(pass_if_truthy(Opcode.STRLEN, "bonus_newstr_len4", [
    *NEWSTR_WORDS(0, b"test"),
    STRLEN_OP(1, 0), LOADC(2, 0), BIN(Opcode.EQ_U, 3, 1, 2)
], 3, consts=(u64(4),)))

# newstr with one char (may add short strings but this is more overhead)
TESTS.append(pass_if_truthy(Opcode.NEWSTR, "newstr_single_char", [
    *NEWSTR_WORDS(0, b"X"),                  # r0 = "X"
    STRLEN_OP(1, 0),
    LOADC(2, 0),
    BIN(Opcode.EQ_U, 3, 1, 2),
], 3, consts=(u64(1),)))

# create "world!" and make sure len is 6
TESTS.append(pass_if_truthy(Opcode.STRLEN, "strlen_basic", [
    *NEWSTR_WORDS(0, b"world!"),             # r0 = "world!"
    STRLEN_OP(1, 0),                         # r1 = strlen(r0) = 6
    LOADC(2, 0),
    BIN(Opcode.EQ_U, 3, 1, 2),
], 3, consts=(u64(6),)))

# double check empty string has length 0
TESTS.append(pass_if_zero(Opcode.STRLEN, "strlen_empty", [
    *NEWSTR_WORDS(0, b""),                   # r0 = ""
    STRLEN_OP(1, 0),                         # r1 = 0
], 1))

# TODO: figure out why it doesn't matter whether strlen is right or not...?
# this passes whether the const is 6 or 7 (STOPPPPPPPPPPPPPPP)
# join "hi" + " guys" and make sure strlen is properly 7
TESTS.append(pass_if_truthy(Opcode.CONCAT, "concat_basic", [
    *NEWSTR_WORDS(0, b"hi"),                # r0 = "hi"
    *NEWSTR_WORDS(1, b" guys"),             # r1 = " guys"
    CONCAT_OP(2, 0, 1),                     # r2 = "hi guys"
    STRLEN_OP(3, 2),                        # r3 = strlen(r2) = 7
    LOADC(4, 0),                            # r4 = u64(7)
    BIN(Opcode.EQ_U, 5, 3, 4),              # r5 = (r3 == r4) (check)
], 5, consts=(u64(7),)))

# make sure a concat with an empty string doesnt fuck w length
TESTS.append(pass_if_truthy(Opcode.CONCAT, "concat_empty_rhs", [
    *NEWSTR_WORDS(0, b"test"),              # r0 = "test"
    *NEWSTR_WORDS(1, b""),                  # r1 = ""
    CONCAT_OP(2, 0, 1),                     # r2 = "test"
    STRLEN_OP(3, 2),                        # r3 = strlen(r2) = 4
    LOADC(4, 0),
    BIN(Opcode.EQ_U, 5, 3, 4),              # (same check as above)
], 5, consts=(u64(4),)))

# concat with empty lhs should preserve rhs length
TESTS.append(pass_if_truthy(Opcode.CONCAT, "bonus_concat_empty_lhs", [
    *NEWSTR_WORDS(0, b""), *NEWSTR_WORDS(1, b"abc"),
    CONCAT_OP(2, 0, 1), STRLEN_OP(3, 2),
    LOADC(4, 0), BIN(Opcode.EQ_U, 5, 3, 4)
], 5, consts=(u64(3),)))

# exercise a minor GC edge case: an old array points at a young string
gc_words = [
    LOADI(1, 1),
    NEWARR(0, Type.OBJ, 1),
]
gc_words += [word for _ in range(100) for word in NEWSTR_WORDS(5, b"x")]
while len(gc_words) < 1030:
    gc_words.append(LOADI(15, 0))
gc_words += [
    *NEWSTR_WORDS(1, b"A"),
    LOADI(2, 0),
    ARRSET(0, 2, 1),
]
gc_words += [word for _ in range(100) for word in NEWSTR_WORDS(5, b"y")]
while len(gc_words) < 2065:
    gc_words.append(LOADI(15, 0))
gc_words += [
    *NEWSTR_WORDS(4, b""),
    ARRGET(3, 0, 2),
    CONCAT_OP(6, 3, 4),
    STRLEN_OP(7, 6),
    LOADC(8, 0),
    BIN(Opcode.EQ_U, 9, 7, 8),
]
TESTS.append(pass_if_truthy(Opcode.CONCAT, "gc_old_to_young_ref", gc_words, 9, consts=(u64(1),)))


# ensure dir then run each test inside
def main() -> None:
    Path("tests").mkdir(exist_ok=True)

    for t in TESTS:
        filename = f"tests/testop{int(t.tag)}_{t.name}.stk"
        with open(filename, "wb") as f:
            # header
            f.write(pack("<4sHHIII", HEADER, VERSION, FLAGS, len(t.words), len(t.consts), len(t.globs)))

            # instructions
            f.write(pack(f"<{len(t.words)}I", *t.words))

            # optional consts/globals
            for c in t.consts: f.write(c)
            for g in t.globs: f.write(g)

        # log if verbose
        if VERBOSE: print(f"Created {filename} ({name})")

if __name__ == "__main__":
    main()
