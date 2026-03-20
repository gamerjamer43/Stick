/**
 * @file errors.h
 * @author Noah Mingolelli
 * @brief yes i really did make another fucking header just for errors. these need to be separate
 * legit just the panic spec and an array containing every error in the order of their panic code
 */
#ifndef ERRORS_H
#define ERRORS_H

typedef enum {
    NO_ERROR = 0,
    PANIC_FILE,
    PANIC_OOB,
    PANIC_NO_HALT,
    PANIC_BAD_MAGIC,
    PANIC_UNSUPPORTED_VERSION,
    PANIC_EMPTY_PROGRAM,
    PANIC_PROGRAM_TOO_BIG,
    PANIC_OOM,
    PANIC_TRUNCATED_CODE,
    PANIC_CONST_READ,
    PANIC_GLOBAL_READ,
    PANIC_REG_LIMIT,
    PANIC_STACK_OVERFLOW,
    PANIC_STACK_UNDERFLOW,
    PANIC_INVALID_CALLABLE,
    PANIC_CALL_FAILED,
    PANIC_TYPE_MISMATCH,
    PANIC_INVALID_OPCODE,
    PANIC_CODE_COUNT
} Panic;

// messages to go with the panic codes (by index)
// TODO: make these more descriptive. and stack tracing lmao
extern const char *const messages[];

#endif