/**
 * @file vm.c
 * @author Noah Mingolelli
 * doccing this later cuz i would rather kill myself rn no cap
 */
#include "vm.h"
#include "io/reader.h"

// listing of all error messages. im making it work then im modularizing. alr prematurely optimized lol
const char *const MESSAGES[] = {
    "",
    "File IO error",
    "Register overflow",
    "No halt",
    "Bad magic",
    "Unsupported version",
    "Empty program",
    "Program too large",
    "Out of memory",
    "Truncated code",
    "Const pool read failed",
    "Globals read failed",
    "Register limit exceeded",
    "Stack overflow",
    "Stack underflow",
    "Invalid callable",
    "Call failed",
    "Type mismatch",
    "Invalid opcode",
    "Arithmetic fault",
};

/**
 * init using struct zeroing
 * @param vm a pointer to an empty vm struct
 */
void vm_init(VM* vm) {
    if (!vm) return;
    *vm = (VM){0};

    // right now this shit is just a leak only allocator...
    // gotta add GC
    heap_init(&vm->heap);

    // TODO: potentially limit registers and frames here (maxregs and maxframes)
}

/**
 * free up everything allocated by the `VM` to not leak memory (like a responsible citizen)
 * @param vm a pointer to a `VM` struct (that already has vm_load used on it)
 */
void vm_free(VM* vm) {
    // if already nulled no worry
    if (vm == NULL) return;

    // free any leftovers
    if (vm->regs) {
        free(vm->regs);
        vm->regs = NULL;
    }

    // free all functions (now stored separately cuz its way safer)
    if (vm->funcs) {
        for (u32 i = 0; i < vm->funccount; i++) {
            free(vm->funcs[i]);
        }
        free(vm->funcs);
        vm->funcs = NULL;
        vm->funccount = 0;
    }

    // free instruction stream (casting to void pointer shuts the compiler up)
    if (vm->istream) {
        free((void*)vm->istream);
        vm->istream = NULL;
        vm->icount  = 0;
    }

    // any dead frames get nulled out
    if (vm->frames) {
        free(vm->frames);
        vm->frames = NULL;
        vm->framecount = 0;
        vm->framecap = 0;
    }


    // the vm will store globals, so we can free them here
    if (vm->globals) {
        free(vm->globals);
        vm->globals = NULL;
        vm->globalcount = 0;
    }

    // free constant pool (cuz i made it owned now)
    if (vm->consts) {
        free((void*)vm->consts);
        vm->consts = NULL;
        vm->constcount = 0;
    }

    // free heap (THIS cleans up any allocations... the GC will do most of that)
    heap_free(&vm->heap);

    vm->ip         = 0;
    vm->panic_code = NO_ERROR;
}

/**
 * mark all heap references reachable from VM roots (registers + globals)
 * call this after heap_begin_gc and before heap_collect
 */
void vm_mark_roots(VM* vm) {
    if (!vm) return;

    // scan active regs
    if (vm->regs && vm->current) {
        u32 top = vm->current->base + vm->current->regc;

        // objects hold heap refs, but callables hold a function pointer
        for (u32 i = 0; i < top; i++) {
            if (vm->regs->types[i] == OBJ) {
                HeapRef ref;
                memcpy(&ref, &vm->regs->payloads[i], sizeof(HeapRef));
                heap_mark_gray(&vm->heap, ref);
            }
        }
    }

    // then scan globals
    if (vm->globals) {
        for (u32 i = 0; i < vm->globalcount; i++) {
            if (vm->globals[i].type == OBJ) {
                HeapRef ref;
                memcpy(&ref, vm->globals[i].val, sizeof(HeapRef));
                heap_mark_gray(&vm->heap, ref);
            }
        }
    }
}

/**
 * runs a minor gc most of the time,
 * escalates to major gc every h->major_interval minors
 */
void vm_gc(VM* vm) {
    if (!vm) return;
    Heap* h = &vm->heap;

    if (h->minor_count >= h->major_interval) {
        vm_gc_major(vm);
    } else {
        vm_gc_minor(vm);
    }
}

/**
 * force a full major collection (all generations)
 */
void vm_gc_major(VM* vm) {
    if (!vm) return;
    heap_begin_major_gc(&vm->heap);
    vm_mark_roots(vm);
    heap_collect(&vm->heap);
}

/**
 * minor collection: only sweep young objects, promote survivors
 */
void vm_gc_minor(VM* vm) {
    if (!vm) return;
    heap_begin_minor_gc(&vm->heap);
    vm_mark_roots(vm);
    heap_minor_collect(&vm->heap);
}


/**
 * load a compiled chunk into the VM instance. the VM takes ownership of the instructions
 * DO NOT REUSE A VM
 * @param vm a pointer to the `VM` to load into
 * @param code `Instruction` stream pointer
 * @param instrcount `Instruction` count
 * @param consts constant pool pointer
 * @param constcount number of constants
 * @param globals_init initial globals (copied into owned storage)
 * @param globalcount number of globals
 */
void vm_load(
    VM* vm,
    const Instruction* code, u32 instrcount,
    const Value* consts, u32 constcount,
    const Value* globals_init, u32 globalcount
) {
    // if no allocated vm ep ep ep bad boy
    if (!vm) return;

    // make sure instructions point to the right thing
    if (vm->istream && vm->istream != code) {
        free((void*)vm->istream);
    }

    // keeps the vm in a safe state in case load fails part way
    vm->istream = NULL;
    vm->icount = 0;
    vm->consts = NULL;
    vm->constcount = 0;
    vm->ip = 0;
    vm->panic_code = NO_ERROR;
    vm->framecount = 0;

    // take ownership of the instructions (no copy, 8 = out of memory)
    if (instrcount > 0) {
        if (!code) {
            vm->panic_code = PANIC_OOM;
            return;
        }

        vm->istream = code;
        vm->icount = instrcount;
    }

    // set constants to provided pool (will be dealt with on load of file)
    vm->consts = consts;
    vm->constcount = constcount;

    // allocate globals (if necessary, if none just return)
    if (globalcount == 0) return;
    vm->globals = (Value*)calloc(globalcount, sizeof(Value));
    if (!vm->globals) {
        vm->panic_code = PANIC_OOM;
        return;
    }

    vm->globalcount = globalcount;
    if (globals_init) {
        memcpy(vm->globals, globals_init, globalcount * sizeof(Value));
    }
}

/**
 * add a frame to the stack
 */
static inline bool push_frame(VM* vm, Frame *frame) {
    if (!vm || !frame) return false;

    // grows will be x2 here too, base of 8
    if (vm->framecount >= vm->framecap) {
        u32 newmax = vm->framecap == 0 ? 8 : vm->framecap * 2;
        if (newmax > MAX_FRAMES) newmax = MAX_FRAMES;

        // already at max capacity
        if (vm->framecount >= newmax) {
            vm->panic_code = PANIC_STACK_OVERFLOW;
            return false;
        }

        // alloc, poll, and set counters properly
        Frame* newframes = (Frame*)realloc(vm->frames, newmax * sizeof(Frame));
        if (!newframes) {
            vm->panic_code = PANIC_OOM;
            return false;
        }
        vm->frames = newframes;
        vm->framecap = newmax;
    }

    // push the frame
    vm->frames[vm->framecount++] = *frame;
    return true;
}

/**
 * pop a frame from the stack
 * @param vm the vm instance
 * @param out optional pointer to store the popped frame
 */
static inline bool pop_frame(VM* vm, Frame *out) {
    if (!vm || vm->framecount == 0) {
        if (vm) vm->panic_code = PANIC_STACK_UNDERFLOW;
        return false;
    }
    vm->framecount--;

    // TODO: look into coroutines, generators, and etc. this is why it's a "pop" not a destruction
    if (out) *out = vm->frames[vm->framecount];
    return true;
}

/**
 * jump to a relative offset. used for JMP, JMPIF, and JMPIFZ. improved safety by casting to int64 (avoiding overflows)
 * @param vm a vm struct to read from
 * @param off the offset to jump forward or backwards from
 */
static inline bool jump_rel(VM* vm, i32 off) {
    i64 next = (i64)vm->ip + (i64)off;

    // (2 = register overflow)
    if (next < 0 || next >= (i64)vm->icount) {
        vm->panic_code = PANIC_OOB;
        return false;
    }

    vm->ip = (u32)next;
    return true;
}

/**
 * copy a value from register to register (nulling does not happen here)
 * @param vm a vm struct to read from
 * @param dest the register to copy to
 * @param src the register to copy from
 */
static inline bool copy(VM* vm, u32 dest, u32 src, u32 offset) {
    // assess the need, based on destination or src, and grow if needed
    u32 need = (dest > src ? dest : src) + vm->current->base + 1;
    if (!ensure_regs(vm, need)) return false;

    // move from source to destination, zero out source register
    vm->regs->types[dest + offset] = vm->regs->types[src + offset];
    vm->regs->payloads[dest + offset] = vm->regs->payloads[src + offset];
    return true;
}

/**
 * when run, if this is a native function it's just called normally (i think maybe i should create a stack frame but TODO)
 * if this is a bytecode function. base is the register index where args start.
 */
bool vm_call(VM *vm, Func *fn, u32 base, u16 argc, u16 reg) {
    if (!vm || !fn) return false;

    switch (fn->kind) {
        case BYTECODE: {
            // validate argc before doing anything
            if (argc != fn->as.bc.argc) return false;

            // ensure frames (calc via base)
            Frame* caller = vm->current;
            u16 new_base = caller->base + caller->regc;
            if (!ensure_regs(vm, new_base + fn->as.bc.regc)) return false;

            // clear the callee register window, then copy incoming args into r0 -> r(argc-1)
            for (u16 i = 0; i < fn->as.bc.regc; i++) {
                vm->regs->types[new_base + i] = NUL;
                vm->regs->payloads[new_base + i].u = 0;
            }
            for (u16 i = 0; i < argc; i++) {
                vm->regs->types[new_base + i] = vm->regs->types[base + i];
                vm->regs->payloads[new_base + i] = vm->regs->payloads[base + i];
            }

            // push frame (safely ofc) and update fp
            Frame callee_frame = {
                .jump = vm->ip,
                .base = new_base,
                .regc = fn->as.bc.regc,
                .reg = reg,
                .callee = fn
            };
            if (!push_frame(vm, &callee_frame)) return false;
            vm->current = &vm->frames[vm->framecount - 1];

            // jump (args are in the first slots so saves us some bullshit)
            vm->ip = fn->as.bc.entry_ip;
            return true;
        }

        // natives r easy legit just a call
        case NATIVE: {
            if (!fn->as.nat.fn || argc != fn->as.nat.argc) return false;
            u32 dest = vm->current->base + reg;
            fn->as.nat.fn(vm, base, argc, dest);
            return true;
        }

        default:
            return false;
    }
}

/**
 * close and free safely on a panic. panics can contain 0 and that is just gonna be a runtime error
 * may just make runtime exception handling sep but i feel like this simplifies handling
 * @param vm pointer to the current vm
 */
u32 vm_panic(u32 code) {
    // quit early
    if (!(code < PANIC_CODE_COUNT)) return code;

    // ansi colors legit arent used anywhere else
    const char* red = "\x1b[31m";
    const char* reset = "\x1b[0m";
    fprintf(
        stderr, "%s[ERROR] Code %d: %s%s\n", 
        red, code, MESSAGES[code], reset
    );
    return code;
}

/**
 * main vm run loop. while ip < icount execute instructions (may move this)
 * potentially look into a dispatch table, as hash lookup would prob speed up some already tight hot loops
 * return false if we do not properly hit a halt, or if we hit panic
 * @param vm the `VM` with instructions loaded into it (vm_load)
 */
bool vm_run(VM* vm) {
    // init checks
    if (!vm || !vm->istream || !vm->regs) return false;

    // default 0, panic = 0 means no errors
    vm->panic_code = NO_ERROR;

    // ensure a base of 16 registers for the entry frame
    if (!ensure_regs(vm, BASE_REGISTERS)) return false;

    // push the initial frame (mark return to the absolute end. deciding if this shud be a panic or if halt should do a return)
    // everything is safe i believe it's just a design choice, but i do not know for certain so i'll doubt myself
    Frame entry = {
        .jump = vm->icount,
        .base = 0,
        .regc = BASE_REGISTERS,
        .callee = NULL
    };
    if (!push_frame(vm, &entry)) return false;
    vm->current = &vm->frames[vm->framecount - 1];

    // gc runs every 1024 instructions just in case
    u32 gc_counter = 0;
    while (vm->ip < vm->icount) {
        if (++gc_counter >= 1024) {
            gc_counter = 0;
            if (heap_should_gc(&vm->heap)) vm_gc(vm);
        }

        // pull current instruction and increment ip
        Instruction ins = vm->istream[vm->ip++];

        if (DEBUG) printf("code: %d\n", opcode(ins));
        switch ((Opcode)opcode(ins)) {
            // normal halt returns with no issues
            case HALT:
                return true;

            // panic on failure, panic returns a code from 0-256 with op_a
            case PANIC:
                vm->panic_code = op_a(ins);
                return false;

            // jump (1 instr, signed)
            case JMP: {
                i32 off = op_signed_i24(ins);
                if (!jump_rel(vm, off)) return false;
                break;
            }

            // dry these up but this should work lol. just check if val isnt zero or false
            case JMPIF: {
                u32 src = op_a(ins) + vm->current->base;
                i32 off = op_signed_i16(ins);

                if (!ensure_regs(vm, src + 1)) return false;

                // if falsy ignore, but if offset invalid, panic
                u8 type = vm->regs->types[src];
                TypedValue payload = vm->regs->payloads[src];
                if (!value_falsy(type, payload)) {
                    if (!jump_rel(vm, off)) return false;
                }
                break;
            }

            case JMPIFZ: {
                u32 src = op_a(ins) + vm->current->base;
                i32 off = op_signed_i16(ins);

                if (!ensure_regs(vm, src + 1)) return false;

                // if falsy ignore, but if offset invalid, panic
                u8 type = vm->regs->types[src];
                TypedValue payload = vm->regs->payloads[src];
                if (value_falsy(type, payload)) {
                    if (!jump_rel(vm, off)) return false;
                }
                break;
            }

            // copy WITHOUT nulling
            case COPY: {
                u32 dest = op_a(ins);
                u32 src  = op_b(ins);

                if (!copy(vm, dest, src, vm->current->base)) return false;
                break;
            }

            // copy AND null source
            case MOVE: {
                u32 dest = op_a(ins);
                u32 src  = op_b(ins);

                if (!copy(vm, dest, src, vm->current->base)) return false;
                vm->regs->types[src + vm->current->base] = 0;
                vm->regs->payloads[src + vm->current->base].u = 0;
                break;
            }

            // load an immediate to a register (16 bit)
            case LOADI: {
                u32 dest  = op_a(ins);
                i32 imm   = op_signed_i16(ins);

                // ensure registers then make the move
                if (!ensure_regs(vm, dest + vm->current->base + 1)) return false;

                // adjust for base and set
                u32 adjusted = dest + vm->current->base;
                vm->regs->types[adjusted] = I64;
                vm->regs->payloads[adjusted].i = imm;
                break;
            }

            // load a constant from the CONSTANT pool
            case LOADC: {
                u32 dest  = op_a(ins);
                u32 index = op_b(ins);

                // if no pool or out of bounds panic
                if (!vm->consts || index >= vm->constcount) {
                    vm->panic_code = PANIC_OOB;
                    return false;
                }

                // ensure registers then make the move
                u32 adjusted = dest + vm->current->base;
                if (!ensure_regs(vm, adjusted + 1)) return false;
                vm->regs->types[adjusted] = vm->consts[index].type;
                memcpy(&vm->regs->payloads[adjusted], vm->consts[index].val, sizeof(u64));
                break;
            }

            // load a global from the pool
            case LOADG: {
                u32 dest  = op_a(ins);
                u32 index = op_b(ins);

                // if no pool or out of bounds panic (really needa fix this name pollution next)
                if (!vm->globals || index >= vm->globalcount) {
                    vm->panic_code = PANIC_OOB;
                    return false;
                }

                // ensure registers then make the move
                u32 adjusted = dest + vm->current->base;
                if (!ensure_regs(vm, adjusted + 1)) return false;
                vm->regs->types[adjusted] = vm->globals[index].type;
                memcpy(&vm->regs->payloads[adjusted], vm->globals[index].val, sizeof(u64));
                break;
            }

            // store a global to the pool
            case STOREG: {
                u32 dest  = op_a(ins);
                u32 index = op_b(ins);

                if (!vm->globals || index >= vm->globalcount) {
                    vm->panic_code = PANIC_OOB;
                    return false;
                }

                // copy with this ugly shite
                u32 adjusted = dest + vm->current->base;
                if (!ensure_regs(vm, adjusted + 1)) return false;
                vm->globals[index].type = vm->regs->types[adjusted];
                memcpy(vm->globals[index].val, &vm->regs->payloads[adjusted], sizeof(u64));
                break;
            }

            // call a function: CALL func_reg argc dest
            case CALL: {
                u32 reg  = op_a(ins);
                u16 argc = op_b(ins);
                u16 dest = op_c(ins);

                // bounds check
                u32 abs = reg + vm->current->base;
                if (!ensure_regs(vm, abs + 1)) return false;

                // yoink from register
                if (vm->regs->types[abs] != CALLABLE) {
                    vm->panic_code = PANIC_INVALID_CALLABLE;
                    return false;
                }

                // extract pointer
                Func* fn = vm->regs->payloads[abs].fn;
                if (!fn) {
                    vm->panic_code = PANIC_INVALID_CALLABLE;
                    return false;
                }

                // pull args and call
                if (!vm_call(vm, fn, abs + 1, argc, dest)) {
                    if (vm->panic_code == 0) vm->panic_code = PANIC_CALL_FAILED;
                    return false;
                }

                break;
            }

            case TAILCALL: {
                u32 reg  = op_a(ins);
                u16 argc = op_b(ins);

                // this register contains the callable
                u32 abs = reg + vm->current->base;
                if (!ensure_regs(vm, abs + 1)) return false;

                // type checks, remove during prod
                // TODO: bytecode verifier!!!!!!!!!
                if (vm->regs->types[abs] != CALLABLE) {
                    vm->panic_code = PANIC_INVALID_CALLABLE;
                    return false;
                }

                Func* fn = vm->regs->payloads[abs].fn;
                if (!fn || fn->kind != BYTECODE) {
                    vm->panic_code = PANIC_INVALID_CALLABLE;
                    return false;
                }

                // validate arg count
                if (argc != fn->as.bc.argc) {
                    vm->panic_code = PANIC_CALL_FAILED;
                    return false;
                }

                // refresh locals (abs + 1 is first arg)
                u32 base = vm->current->base;
                for (u16 i = 0; i < fn->as.bc.regc; i++) {
                    vm->regs->types[base + i] = NUL;
                    vm->regs->payloads[base + i].u = 0;
                }
                for (u16 i = 0; i < argc; i++) {
                    vm->regs->types[base + i] = vm->regs->types[abs + 1 + i];
                    vm->regs->payloads[base + i] = vm->regs->payloads[abs + 1 + i];
                }

                // overwrite frame data and jump back to start
                // (this is a recursive call without a push)
                vm->current->callee = fn;
                vm->current->regc = fn->as.bc.regc;
                vm->ip = fn->as.bc.entry_ip;
                break;
            }

            // return from function: RET register
            case RET: {
                u32 ret = op_a(ins);
                u32 abs = ret + vm->current->base;

                // get return val then pop
                Frame popped;
                Value returned = {0};
                if (abs < MAX_REGISTERS) {
                    returned.type = vm->regs->types[abs];
                    memcpy(returned.val, &vm->regs->payloads[abs], sizeof(u64));
                }
                
                // if pop somehow failed GET OUT.
                if (!pop_frame(vm, &popped)) return false;

                // jump ip back and restore previous state
                if (vm->framecount == 0) return true;
                vm->current = &vm->frames[vm->framecount - 1];
                vm->ip = popped.jump;

                // store return value in caller spec
                u32 adjusted = vm->current->base + popped.reg;
                vm->regs->types[adjusted] = returned.type;
                memcpy(&vm->regs->payloads[adjusted], returned.val, sizeof(u64));
                break;
            }

            case NEWARR: {
                u32 dest = op_a(ins) + vm->current->base;
                u8  elem_type = op_b(ins);
                u32 cap_reg = op_c(ins) + vm->current->base;

                if (LIKELYFALSE(!ensure_regs(vm, (dest > cap_reg ? dest : cap_reg) + 1))) return false;

                // must be a u64 or i64 (which can be coerced)
                u64 cap;
                COERCE_U64(vm, cap_reg, cap);

                // allocate on heap and store its HeapRef as OBJ @ dest
                HeapRef ref = heap_alloc_array(&vm->heap, elem_type, (u32)cap);
                STORE_HEAP_RESULT(vm, dest, ref);
                break;
            }

            case ARRGET: {
                u32 base = vm->current->base;
                u32 dest    = op_a(ins) + base;
                u32 arr_reg = op_b(ins) + base;  // src1 is where the array lives
                u32 idx_reg = op_c(ins) + base;  // src2 is the index

                // make sure no reaches are out of bounds with one simple check
                u32 max = dest;
                if (LIKELYFALSE(arr_reg > max)) max = arr_reg;
                if (LIKELYFALSE(idx_reg > max)) max = idx_reg;
                if (LIKELYFALSE(!ensure_regs(vm, max + 1))) return false;

                // deref to access the actual array (will be done by bytecode verifier later)
                HeapArray* arr;
                DEREF_HEAP(vm, arr_reg, HEAP_TYPE_ARRAY, HeapArray, arr);

                // index coercion (i64 or u64)
                u64 idx;
                COERCE_U64(vm, idx_reg, idx);

                // ensure the array index isnt out of bounds either
                if (LIKELYFALSE(idx >= arr->length)) { vm->panic_code = PANIC_OOB; return false; }

                // read element into dest (and clear to avoid stale bs)
                vm->regs->types[dest] = arr->elem_type;
                vm->regs->payloads[dest].u = 0;
                memcpy(&vm->regs->payloads[dest], (u8*)arr->data + idx * arr->elem_size, arr->elem_size);
                break;
            }

            case ARRSET: {
                u32 base = vm->current->base;

                // src0 is the where the array lives, src1 is the index, 
                // src2 is a 0-256 val of a register to copy in there
                u32 arr_reg = op_a(ins) + base;
                u32 idx_reg = op_b(ins) + base;
                u32 val_reg = op_c(ins) + base;

                // the rest is basically the same as above
                u32 max = arr_reg;
                if (idx_reg > max) max = idx_reg;
                if (val_reg > max) max = val_reg;
                if (LIKELYFALSE(!ensure_regs(vm, max + 1))) return false;

                // if it's not an obj panic
                if (vm->regs->types[arr_reg] != OBJ) {
                    vm->panic_code = PANIC_TYPE_MISMATCH;
                    return false;
                }

                // deref and check if its an array (or panic)
                HeapRef arr_ref;
                memcpy(&arr_ref, &vm->regs->payloads[arr_reg], sizeof(HeapRef));
                if (LIKELYFALSE(heapref_type(arr_ref) != HEAP_TYPE_ARRAY)) {
                    vm->panic_code = PANIC_TYPE_MISMATCH;
                    return false;
                }

                HeapArray* arr = (HeapArray*)heap_deref(&vm->heap, arr_ref);
                if (LIKELYFALSE(!arr)) {
                    vm->panic_code = PANIC_OOB;
                    return false;
                }

                // coerce index into a u64
                u64 idx;
                COERCE_U64(vm, idx_reg, idx);

                // allow writes up to capacity (not just length)
                if (LIKELYFALSE(idx >= arr->capacity)) { vm->panic_code = PANIC_OOB; return false; }

                // value type must match element type
                if (vm->regs->types[val_reg] != arr->elem_type) {
                    vm->panic_code = PANIC_TYPE_MISMATCH;
                    return false;
                }

                // write element and bump length if appending
                memcpy((u8*)arr->data + idx * arr->elem_size, &vm->regs->payloads[val_reg], arr->elem_size);
                if (idx + 1 > arr->length) arr->length = (u32)(idx + 1);
                heap_mark_dirty(&vm->heap, arr_ref);
                break;
            }

            case ARRLEN: {
                u32 base = vm->current->base;
                u32 dest    = op_a(ins) + base;
                u32 arr_reg = op_b(ins) + base;

                // TODO: bytecode verifier and get these checks the fuck out!
                u32 max = dest > arr_reg ? dest : arr_reg;
                if (LIKELYFALSE(!ensure_regs(vm, max + 1))) return false;

                HeapArray* arr;
                DEREF_HEAP(vm, arr_reg, HEAP_TYPE_ARRAY, HeapArray, arr);

                // store array length in the destination register (and type properly)
                vm->regs->types[dest] = U64;
                vm->regs->payloads[dest].u = (u64)arr->length;
                break;
            }

            case CONCAT: {
                u32 dest, lhs, rhs;
                if (!binop_indices(vm, ins, &dest, &lhs, &rhs)) return false;

                // create 2 heap string pointers and deref
                HeapString* sa;
                HeapString* sb;
                DEREF_HEAP(vm, lhs, HEAP_TYPE_STRING, HeapString, sa);
                DEREF_HEAP(vm, rhs, HEAP_TYPE_STRING, HeapString, sb);

                // build a concat buffer
                u32 new_len = sa->length + sb->length;
                char* buf = (char*)malloc(new_len + 1);
                if (LIKELYFALSE(!buf)) { vm->panic_code = PANIC_OOM; return false; }

                // copy both into the buffer at proper indices
                memcpy(buf, sa->data, sa->length);
                memcpy(buf + sa->length, sb->data, sb->length);
                buf[new_len] = '\0';

                // allocate a new string, then free the old buffer
                HeapRef ref = heap_alloc_string(&vm->heap, buf, new_len);
                free(buf);

                // store and check for OOM w this nice helper
                STORE_HEAP_RESULT(vm, dest, ref);
                break;
            }

            case STRLEN: {
                u32 base = vm->current->base;
                u32 dest = op_a(ins) + base;
                u32 src  = op_b(ins) + base;

                u32 need = (dest > src ? dest : src) + 1;
                if (LIKELYFALSE(!ensure_regs(vm, need))) return false;

                // simple easy fun, deref string
                HeapString* s;
                DEREF_HEAP(vm, src, HEAP_TYPE_STRING, HeapString, s);

                // store its type in specified dest reg (adjusted for base ofc) as U64
                vm->regs->types[dest] = U64;
                vm->regs->payloads[dest].u = (u64)s->length;
                break;
            }

            case NEWSTR: {
                u32 dest = op_a(ins) + vm->current->base;
                u32 len  = (u32)op_unsigned_u16(ins);

                if (LIKELYFALSE(!ensure_regs(vm, dest + 1))) return false;

                // a bunch of 4 byte words follow this instruction with info on the string
                u32 nwords = (len + 3) / 4;
                if (LIKELYFALSE(vm->ip + nwords > vm->icount)) {
                    vm->panic_code = PANIC_OOB;
                    return false;
                }

                // read those bytes from istream
                char* buf = (char*)malloc(len + 1);
                if (LIKELYFALSE(!buf)) { vm->panic_code = PANIC_OOM; return false; }

                for (u32 i = 0; i < nwords; i++) {
                    u32 word = vm->istream[vm->ip + i];
                    u32 remaining = len - i * 4;
                    u32 to_copy = remaining < 4 ? remaining : 4;
                    memcpy(buf + i * 4, &word, to_copy);
                }

                // properly null term and adjust instruction pointer properly
                buf[len] = '\0';
                vm->ip += nwords;

                HeapRef ref = heap_alloc_string(&vm->heap, buf, len);
                free(buf);
                STORE_HEAP_RESULT(vm, dest, ref);
                break;
            }

            // cast helpers
            case I2D: CAST_TYPED(I64, i, DOUBLE, d, (double)vm->regs->payloads[src].i); break;
            case I2F: CAST_TYPED(I64, i, FLOAT,  f, (float)vm->regs->payloads[src].i);  break;
            case D2I: CAST_TYPED(DOUBLE, d, I64, i, (i64)vm->regs->payloads[src].d);    break;
            case F2I: CAST_TYPED(FLOAT,  f, I64, i, (i64)vm->regs->payloads[src].f);    break;
            case I2U: CAST_TYPED(I64, i, U64, u, (u64)vm->regs->payloads[src].i);       break;
            case U2I: CAST_TYPED(U64, u, I64, i, (i64)vm->regs->payloads[src].u);       break;
            case U2D: CAST_TYPED(U64, u, DOUBLE, d, (double)vm->regs->payloads[src].u); break;
            case U2F: CAST_TYPED(U64, u, FLOAT,  f, (float)vm->regs->payloads[src].u);  break;
            case D2U: CAST_TYPED(DOUBLE, d, U64, u, (u64)vm->regs->payloads[src].d);    break;
            case F2U: CAST_TYPED(FLOAT,  f, U64, u, (u64)vm->regs->payloads[src].f);    break;

            // all operators muahahaha (i think) 
            // binary ops, arithmetic and bitwise (default to signed 64-bit)
            case ADD:   BINOP_I64(+);  break;
            case SUB:   BINOP_I64(-);  break;
            case MUL:   BINOP_I64(*);  break;
            case DIV:   BINOP_I64_SAFE_DIV(); break; // potentially remove and enforce div by 0 at compiler level
            case MOD:   BINOP_I64_SAFE_MOD(); break;
            case AND:   BINOP_I64(&);  break;
            case OR:    BINOP_I64(|);  break;
            case XOR:   BINOP_I64(^);  break;
            case SHL:   BINOP_I64(<<); break;

            // SAR AND SHR BOTH THE SAME. FIGURE OUT WAG1
            case SHR:   BINOP_I64(>>); break;

            // unsigned ops (u64)
            case ADD_U: BINOP_U64(+);  break;
            case SUB_U: BINOP_U64(-);  break;
            case MUL_U: BINOP_U64(*);  break;
            case DIV_U: BINOP_U64_SAFE_DIV(); break; // potentially remove and enforce div by 0 at compiler level
            case MOD_U: BINOP_U64_SAFE_MOD(); break;
            case AND_U: BINOP_U64(&);  break;
            case OR_U:  BINOP_U64(|);  break;
            case XOR_U: BINOP_U64(^);  break;
            case SHL_U: BINOP_U64(<<); break;
            case SHR_U: BINOP_U64(>>); break;
            
            // boolean comparison ops
            case EQ:    CMPOP_I64(==); break;
            case NEQ:   CMPOP_I64(!=); break;
            case GT:    CMPOP_I64(>);  break;
            case GE:    CMPOP_I64(>=); break;
            case LT:    CMPOP_I64(<);  break;
            case LE:    CMPOP_I64(<=); break;

            // u64 comparisons
            case EQ_U:  CMPOP_U64(==); break;
            case NEQ_U: CMPOP_U64(!=); break;
            case GT_U:  CMPOP_U64(>);  break;
            case GE_U:  CMPOP_U64(>=); break;
            case LT_U:  CMPOP_U64(<);  break;
            case LE_U:  CMPOP_U64(<=); break;

            // float ops (f32)
            case ADD_F: BINOP_F32(+);  break;
            case SUB_F: BINOP_F32(-);  break;
            case MUL_F: BINOP_F32(*);  break;
            case DIV_F: BINOP_F32(/);  break;
            case EQ_F:  CMPOP_F32(==); break;
            case NEQ_F: CMPOP_F32(!=); break;
            case GT_F:  CMPOP_F32(>);  break;
            case GE_F:  CMPOP_F32(>=); break;
            case LT_F:  CMPOP_F32(<);  break;
            case LE_F:  CMPOP_F32(<=); break;

            // float ops (f64)
            case ADD_D: BINOP_F64(+);  break;
            case SUB_D: BINOP_F64(-);  break;
            case MUL_D: BINOP_F64(*);  break;
            case DIV_D: BINOP_F64(/);  break;
            case EQ_D:  CMPOP_F64(==); break;
            case NEQ_D: CMPOP_F64(!=); break;
            case GT_D:  CMPOP_F64(>);  break;
            case GE_D:  CMPOP_F64(>=); break;
            case LT_D:  CMPOP_F64(<);  break;
            case LE_D:  CMPOP_F64(<=); break;

            // unary ops
            case NEG:    UNOP_I64(-);  break;
            case NEG_U:  UNOP_U64(-);  break;
            case NEG_F:  UNOP_F32(-);  break;
            case NEG_D:  UNOP_F64(-);  break;
            case BNOT:   UNOP_I64(~);  break;
            case BNOT_U: UNOP_U64(~);  break;
            
            // logical not has special cases. ONLY can be used on boolean values. gonna fix jmpif and jmpifz to be the same mayb
            // also prolly gonna figure out a way to fucking dry this cuz its just a type check
            case LNOT: {
                u32 src = op_a(ins) + vm->current->base;

                if (!ensure_regs(vm, src + 1)) return false;
                if (vm->regs->types[src] != BOOL) { 
                    vm->panic_code = PANIC_TYPE_MISMATCH; 
                    return false; 
                }
                
                vm->regs->payloads[src].u = vm->regs->payloads[src].u ? 0u : 1u;
                break;
            }

            // nothing matched (9 = invalid opcode)
            default: {
                vm->panic_code = PANIC_INVALID_OPCODE;
                return false;   
            }
        }
    }

    // (3 = no halt found)
    vm->panic_code = PANIC_NO_HALT;
    return false;
}

/**
 * main loop driving this big boy. gonna figure out how to properly modularize next
 */
int main(int argc, char const *argv[]) {
    // default to program.stk if no path for rn. no emitter so im just writing test binaries using python's struct.pack
    const char* path = (argc > 1) ? argv[1] : NULL;
    if (path == NULL) {
        printf("provide a compiled .stk file to run\n");
        exit(0);
    }

    // init and load file
    VM vm;
    vm_init(&vm);

    // if failed free safely, return panic code or 1 if no code
    if (!vm_load_file(&vm, path)) {
        printf("error loading %s, code: %u\n", path, vm.panic_code);
        vm_free(&vm);
        return vm.panic_code ? (int)vm.panic_code : 1;
    }

    // check instructions first
    if (vm.istream && vm.icount > 0) {
        printf("Code: [");
        for (u32 i = 0; i < vm.icount - 1; i++) {
            printf("0x%08" PRIX32 ",", vm.istream[i]);
        }
        printf("0x%08" PRIX32 "]\n", vm.istream[vm.icount - 1]);
    } 
    else printf("Code: []\n");

    // run returns a status (false with code set if failed)
    bool ok = vm_run(&vm);
    u32 code = vm.panic_code;

    // free everything safely when done, log any errors
    vm_free(&vm);
    if (!ok && code != 0) vm_panic(code);
    return (int)code;
}
