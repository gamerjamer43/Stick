/**
 * @file opcodes.h
 * @author Noah Mingolelli
 * @brief the opcode table is so long that i wanted to make a sep file
 *
 * read what each one does and how i have it laid out. i commented under each to avoid a giant docstring.
 */

#ifndef OPCODES_H
#define OPCODES_H

typedef enum {
    /**
     * to make things easier
     *
     * order of ops will always be denoted
     * [op][src0][src1][src2]
     */

    HALT = 0,  // stop program execution

    // runtime errors
    PANIC,     // abort execution (attach an error code thru a constant)

    // move instruction pointer
    JMP,       // offset instruction pointer by a signed 32 bit value
    JMPIF,     // if src0 != 0, IP += (i32)src1
    JMPIFZ,    // if src0 == 0, IP += (i32)src1

    // movement and storage
    COPY,      // copy a value from one register to another (no nulling)
    MOVE,      // move values between registers (null the previous register)
    LOADI,     // load an immediate value -32768 to 32767
    LOADC,     // load constant into src0     C = constant
    LOADG,     // load global into src0
    STOREG,    // store global into the table G = global

    // CURRENTLY DONE UP TO HERE (WITH THE EXCEPTION OF JMPIF)

    // call stack
    CALL,      // create a stack frame and jump
    TAILCALL,  // reuse current stack frame for another call
    RET,       // return to caller

    // bitwise (and also logical operators as bools make false 0)
    AND,       // bitwise and src1 by src2 and store in src0
    OR,        // bitwise or src1 by src2 and store in src0
    XOR,       // bitwise exclusive or src1 by src2 and store in src0
    LNOT,      // perform a logical negation on src0 (unary)
    BNOT,      // perform a bitwise negation on src0 (unary)

    // shifts
    SHL,       // shift left src1 by src2 and store in src0 (signed ints only)
    SHR,       // shift right src1 by src2 and store in src0 (signed ints only)
    SAR,       // arithmetic shift right src1 by src2 and store in src0 (signed ints only)

    // heap
    NEWARR,    // dst = new array(length)
    NEWTABLE,  // dst = new hashmap
    NEWOBJ,    // dst = instance/struct with fields

    // tables
    GETELEM,   // dst = table[key] (nil if missing)
    SETELEM,   // table[key] = value

    // arrays
    ARRGET,    // dst = arr[idx]
    ARRSET,    // arr[idx] = val
    ARRLEN,    // dst = length(arr)

    // strings
    CONCAT,    // dst = str(src1) + str(src2)
    STRLEN,    // dst = length(str) MAY REMOVE TO TREAT STRINGS AS ARRAYS OF CHARS

    // conversions
    I2D,       // int to double. dst = (double)src1
    I2F,       // int to float. dst = (float)src1
    D2I,       // double to int. dst = (i64)src1
    F2I,       // float to int. dst = (i64)src1
    I2U,       // signed to unsigned 64 bit. dst = (u64)src1
    U2I,       // unsigned to signed 64 bit. dst = (i64)src1
    U2D,       // unsigned int to double
    U2F,       // unsigned int to float
    D2U,       // double to unsigned int
    F2U,       // float to unsigned int

    // arithmetic operators
    ADD,       // add src1 and src2 and store in the src0
    SUB,       // do src1 minus src2 and store in the src0
    MUL,       // multiply src1 and src2 and store in src0
    DIV,       // do src1 divided by src2 and store in src0 (signed ints)
    MOD,       // do src1 mod src2 and store in src0 (signed ints)
    NEG,       // unary negation. multiply the value in src0 by -1

    // logical operators
    EQ,        // src0 = (src1 == src2)
    NEQ,       // src0 = (src1 != src2)
    GT,        // src0 = (src1 >  src2) (signed)
    GE,        // src0 = (src1 >= src2) (signed)
    LT,        // src0 = (src1 <  src2) (signed)
    LE,        // src0 = (src1 <= src2) (signed)

    // the same set but for every type
    // unsigned 64-bit
    ADD_U, SUB_U, MUL_U, DIV_U, MOD_U,
    NEG_U,
    EQ_U, NEQ_U, GT_U, GE_U, LT_U, LE_U,

    // 32-bit float
    ADD_F, SUB_F, MUL_F, DIV_F,
    NEG_F,
    EQ_F, NEQ_F, GT_F, GE_F, LT_F, LE_F,

    // 64-bit float
    ADD_D, SUB_D, MUL_D, DIV_D,
    NEG_D,
    EQ_D, NEQ_D, GT_D, GE_D, LT_D, LE_D,

    // typed bitwise ops (integer only)
    AND_U, OR_U, XOR_U, SHL_U, SHR_U, BNOT_U,

    // more here
} Opcode;

#endif