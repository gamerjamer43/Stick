#include "reader.h"

bool read_exact(FILE* f, void* out, size_t n) {
    return fread(out, 1, n, f) == n;
}

u16 read_u16_le(const u8 b[2]) {
    return (u16)b[0] | ((u16)b[1] << 8);
}

u32 read_u32_le(const u8 b[4]) {
    return pack(b[3], b[2], b[1], b[0]);
}


// won't have to edit much. it seems to be working just fine, const and global pool is all good (though all are fixed at a 4 byte count max)
// so legitimately just widen some counts and that's all. maybe do a 32 byte header:
// <4 byte magic> <2 byte version> <2 byte flags> <8 byte instruction count> <8 byte constant count> <8 byte global count>
bool vm_load_file(VM* vm, const char* path) {
    if (!vm || !path) return false;

    // any io errors stop here (1 = file io error)
    FILE* f = fopen(path, "rb");
    if (!f) {
        vm->panic_code = PANIC_FILE;
        return false;
    }

    // no error or anything embedded in the program loaded yet
    int err = 0;
    Instruction* code = NULL;
    Func** funcs = NULL;
    Value* consts = NULL;
    Value* globals = NULL;

    // header shit (3 = truncated header, make sure it's exactly 20 bytes).
    // it goes: STIK <2 byte version> <2 byte flags> <4 byte instruction count> <4 byte constant count> <4 byte global count>
    u8 magic[4], vbuf[2], fbuf[2], icount[4], ccount[4], gcount[4];

    if (
        !read_exact(f, magic, 4)  ||
        !read_exact(f, vbuf, 2)   ||
        !read_exact(f, fbuf, 2)   ||

        // TODO: look into making icount 8 bytes
        !read_exact(f, icount, 4) ||
        !read_exact(f, ccount, 4) ||
        !read_exact(f, gcount, 4)
    ) {
        err = PANIC_BAD_MAGIC;
        goto fail_file;
    }

    // ensure magic first (4 = bad magic)
    if (memcmp(magic, MAGIC, 4) != 0) {
        err = PANIC_BAD_MAGIC;
        goto fail_file;
    }

    // pulled from the file
    u16 version     = read_u16_le(vbuf);
    u32 count       = read_u32_le(icount);
    u32 constcount  = read_u32_le(ccount);
    u32 globalcount = read_u32_le(gcount);

    // also pulled but unused frn i'll let the compiler keep warning me
    u16 flags       = read_u16_le(fbuf);

    // version check (5 = unsupported version)
    // versions should be backwards compatible
    if (version > VERSION) {
        err = PANIC_UNSUPPORTED_VERSION;
        goto fail_file;
    }

    // sanity checks (6 = empty. empty programs are errors for rn, will write a halt later)
    if (count == 0) {
        err = PANIC_EMPTY_PROGRAM;
        goto fail_file;
    }

    // prevent overflow in malloc (4b instrs = failure)
    // TODO: read line 39 nincompoop
    if (count > (UINT32_MAX / (u32)sizeof(Instruction))) {
        err = PANIC_PROGRAM_TOO_BIG;
        goto fail_file;
    }

    // setup instruction pointer
    code = (Instruction*)malloc((size_t)count * sizeof(Instruction));
    if (!code) {
        err = PANIC_OOM;
        goto fail_file;
    }

    // read instructions into the buffer (using little endian by default) and safely null file when done
    // ensure real count and listed count of instructions match up (9 = truncated code)
    size_t got = fread(code, sizeof(Instruction), count, f);
    if (got != count) {
        err = PANIC_TRUNCATED_CODE;
        goto fail_code;
    }

    // init registers
    vm->regs = (Registers*)calloc(1, sizeof(Registers));
    if (!vm->regs) {
        vm->panic_code = PANIC_OOM;
        goto fail_code;
    }

    // MAY EDIT UP THIS SECTION, A DRY FEELS VERY NECESSARY
    // read constant pool after code
    if (constcount > 0) {
        consts = (Value*)malloc(constcount * sizeof(Value));
        if (!consts || !read_exact(f, (u8*)consts, constcount * sizeof(Value))) {
            err = PANIC_CONST_READ;
            goto fail_code;
        }

        // allocate function table sized to constants so we can patch callables
        funcs = (Func**)calloc(constcount, sizeof(Func*));
        if (!funcs) {
            err = PANIC_OOM;
            goto fail_code;
        }
        
        // turn packed callables into function pointers at load time (avoids runtime bs for a little startup delay whateverrrrr)
        for (u32 i = 0; i < constcount; i++) {
            if (consts[i].type != CALLABLE) continue;

            // copy vals out
            u32 entry_ip;
            u16 argc, regc;
            memcpy(&entry_ip, &consts[i].val[0], sizeof(u32));
            memcpy(&argc, &consts[i].val[4], sizeof(u16));
            memcpy(&regc, &consts[i].val[6], sizeof(u16));
            
            // malloc and poll
            Func* fn = (Func*)malloc(sizeof(Func));
            if (!fn) {
                err = PANIC_OOM;
                goto fail_code;
            }

            // fill function, then patch old callable space to store the new function pointer
            // i could do this with pointer packing but i have 8 bits so i don't necessarily need to?
            fn->kind = BYTECODE;
            fn->as.bc.entry_ip = entry_ip;
            fn->as.bc.argc = argc;
            fn->as.bc.regc = regc;
            
            funcs[i] = fn;
            memcpy(consts[i].val, &fn, sizeof(Func*));
            vm->funccount++;
        }
    }
    vm->funcs = funcs;

    // lastly the global pool
    if (globalcount > 0) {
        globals = (Value*)malloc(globalcount * sizeof(Value));
        if (!globals || !read_exact(f, (u8*)globals, globalcount * sizeof(Value))) {
            err = PANIC_GLOBAL_READ;
            goto fail_code;
        }
    }
    // END POTENTIAL EDIT

    // close safely and null out
    fclose(f);
    f = NULL;

    // vm load deals with the rest
    vm_load(vm, code, count, consts, constcount, globals, globalcount);
    if (!vm->istream) {
        err = vm->panic_code ? (int)vm->panic_code : PANIC_OOM;
        goto fail_code;
    }

    // free moved globals
    if (globals) {
        free(globals);
        globals = NULL;
    }

    // TODO: load constant and global pools next
    // vm->file_flags = flags; will deal with storing flags later

    return true;

// frees code on an error (then falls thru to fail file)
fail_code:
    free(code);

    // free associated pools
    if (consts) {
        free(consts);
        consts = NULL;
    }

    if (globals) {
        free(globals);
        globals = NULL;
    }

    if (funcs) {
        for (u32 i = 0; i < constcount; i++) {
            free(funcs[i]);
        }
        free(funcs);
        vm->funcs = NULL;
        vm->funccount = 0;
    }
    
    if (vm->regs) {
        free(vm->regs);
        vm->regs = NULL;
    }

// errors before the code exists or after the code is freed
fail_file:
    if (f) fclose(f);
    vm->panic_code = err;
    return false;
}
