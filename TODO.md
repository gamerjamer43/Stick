if anyone has recommendations feel free to hit me up @ [noah@noahmingolel.li](mailto:noah@noahmingolel.li) like it 1996.

## future execution features:
- JIT step (HARD. have to interoperate with C.)
- web embed (use cheerp its so light. should be only a 20kb runtime which is fucking insane)
- fully aot compiled (replacing cheerp with a wasm compiled backend using LLVM and/or cranelift. may do both to target as much as possible)

## backlog
asap:
- for analyzer, do:
  - function calls
  - function decls
  - returns
  - reassignments
  - code cleaning

- for vm do:
  - setup heap to use forwarding pointers (and figure out how to do that)
  - heap caching (heavily used heap objects should load from a higher precedence array, or even actually cache the line)
  - opcodes:
    - TAILCALL (duh its listed 3 times its just cool to me)
    - NEWARR, NEWTABLE, NEWOBJ (when heap done, maybe add a raw ALLOC too)
    - GETELEM, SETELEM (hashtable ops)
    - ARRGET, ARRSET, ARRLEN (array ops)
    - CONCAT, STRLEN (string ops)

decisions:
- decide on lookup model (bitmap seems fine, but i need to know the max U, so prob hashmap)
- decide on another potential Value conversion (do i want to suck up the padding after all that fucking work to store some shit in the remaining 7 bytes? this is only for the constant/global pool though, so only pre compilation)
- decide on calling convention (still need to finalize. as of rn frames slot their args in the front registers, i could dedicate some, and i havent tested c native calling but it is pretty basic so should work. *out might contain pointer structures tho so i have to figure THAT out too... prolly the 2nd most complex besides header/heap and the most detail intensive)
- decide on numeric types:
  - how will i offer <64 bit ints, VM level or compiler level
  - how will i offer >64 bit ints, 2 sequential slots or a pointer to a heap instance
  - should i offer arbitrarily sized ints?
- research how to get source references down to bytecode without bloating it
- double check frame for opts (they work fine as is so idk what else besides reducing register swaps we can do)

implementations (everything i can think of):
- BYTECODE VERIFIER. this way we can avoid a bunch of runtime checks due to any issues being found BEFORE THEN!!!!
- include vs import:
  - include -> **include** the 
- vm dispatch loop (finalize some things to reduce cycle count and mem footprint. a lot of memcpys due to 9 byte vals in const/glob pool. might fucking plow thru all my work)
- closures/upvalues/coroutines (scope escaping methods, i dont entirely understand them but we'll learn! we always do!)
- native/ffi hooks (already have the basic callables just need to figure out how to call from the bytecode, and if the C function is stored in there and compiled on first run or if it comes from lib, which is dealt with at compile time)
- string interning (simple. just if a string is alr on the heap and you make another instance of it in a loop/a different instance uses the same value we use the interned string)