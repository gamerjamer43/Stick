# Language Name: Stick
### Background. i called it stick for many reasons:
- pokes with stick cmonnnnnnnnn do something
- light like a stick
- easy to wield like a stick
- embeddable (into the ground) like a stick
- maybe the name will *stick* HAHAHAHAHA

# Commit 1:
- opcodes.h (created):
  - Set up the Opcodes table (which is just an enum, each opcode is an int, halt being 0 and every one being sequential)
- typing.h (created):
  - Added a type enum, basically just a container holding all native types
  - Set up basic Values (which just are basically containers with an attatched type and data)
  - Set up some basic function types for both bytecode and native (just a pointer to a native function w some extra necessary data) functions
- vm.h (created):
  - Established truthiness with basic types (based on what type Value contains)
  - Set up how we handle instructions (u32, pulling different sections of u8 from the u32, as its packed)
  - Forward declared object types we may need (stubbed for rn)
  - Set up basic stack frame handling, which seems simple but has been rly hard for me to fully grasp lol.
  - Set up VM with some shit we may need. Added instruction stream, const pools, program globals, frame management, and a panic code (in case we exit with errors)
  - Also added a basic API of functions I will commonly use.
- LICENSE (created):
  - licensed with GPLv3 (if you wanna really use or rework this you gotta credit me for making it lol)

# Commit 2:
- vm.h:
  - added doxygen tags to header (legit this is all i did this commit)
- typing.h:
  - same as above
  - changed up some names (FuncKind -> FuncType, moved Func forward decl to a proper spot too)
- opcodes.h:
  - same as above. no docs yet

# Commit 3:
- vm.c (created):
  - added basic init and free state given the api in the header
- vm.h:
  - docced completely (kill me)
  - dicked with names again (arity (common but ugly name) -> argc, code -> istream, code_len/const_len -> codecount/constcount)
  - made truthiness with functions work off what returns from the call
  - added vm_call (function calls) and vm_load (load the entire binary properly into the VM, setup IP and all other shit)
- typing.h:
  - added IField (legit just u8)
  - moved shit around so it would actually compile (i forgot to forward decl VM above everything)
- MAKEFILE (created)
  - to make my life easier

# Commit 3.5 (not even commit 4):
- vm.c:
  - added logs to make sure init and free worked with -O3
- vm.h:
  - basically nothing. straightened out docs and added #include <stdio.h> for logs

# Commit 4:
- vm.h:
  - renamed some instruction based shit (pack, opcode, op_a, op_b, op_c)
  - added a globals owned flag for some reason (the vm will own all globals, and is responsible for freeing them)
- typing.h:
  - built ObjString and ObjArray representations
  - have to figure out how to do maps/tables and a generic Obj
- vm.c:
  - added instruction logging
  - vm_load copies everything provided by the "binary" (just an in memory array, will make it load from an ACTUAL FILE next)
  - vm_run steps through each provided instruction and runs a match statement based on opcode.
  - only added support for HALT, PANIC, and default failures.

# Commit 5:
- test.py (created):
  - temp file to just write a header (STIK for right now), and known opcodes to a binary file containing bytecode, just gonna test HALT and PANIC still.
  - no: global or const pool yet
- typing.h:
  - note on the forward decl i left (for rn) attached to Value
- opcodes.h:
  - another small note on potentially treating strings as char arrays instead of their own obj
- vm.h:
  - added array LEN helper, returns a u32 (and used a macro to avoid pointer decay)
  - tightened docs v minorly
- vm.c:
  - remove the test we had in favor of loading from a written binary
  - small changes to make code loaded into a VM owned buffer, and to deal with that properly in free.
  - a small load helper (read a specific amt of bits)
  - load instructions using a file helper (which i will be moving out along with the load helper, just want this to work then i'll deal w the rest)
  - used the only gotos i want to use lol, centralizes freeing and closing the file on error
- log.md (created):
  - wrote this part of the commit log

# Commit 5.5:
- vm.c:
  - moved reader helpers to io/reader.c, specs in io/reader.h
  - moved everything for vm (opcodes.h, typing.h, vm.h, vm.c) into vm/
- reader.h (created):
  - did not have function prototypes before, now i do
- reader.c:
  - moved read_exact, read_u16_le, read_u32_le, and vm_load_file inside
- MAKEFILE:
  - updated to properly use -I, now compiling whats in vm/ and io/

---

Skipping to 10. I'll doc/log later. Too busy CODING.

---

# Commit 10:
### MINOR:
- LICENSE:
  - minor update to the description
- Makefile:
  - removed -g for speed testing
- opcodes.h:
  - added logical not (bitwise not negates all bits durrrr)
- typing.h:
  - added registers file (legit 2 arrays. so we don't have to fight w memcpys or alignment. 8 bytes then 1 byte for alignment)
  - changed some types around (macro defined types -> enum, defined some more macros for register/frame scaling)
  - added base arg (the base register for this functions storage) to NativeFn
- vm.h:
  - name cleaning (i dont wanna list all of them just a lot)
  - added a pointer to the current frame (idk why i didnt have this b4)
  - implemented the register file into vm (and need to prolly remove some unused shit)
  - a macro (3 but i copied it) for binops comp ops and unary ops
- reader.c:
  - setup registers on load. will expand if needed but thats TBD

### MAJOR
- test.py:
  - added 35 new tests to test every facet of this. i added a lot of operators so i need to test those, and additionally i added some bonuses
- runner.py:
  - a unified place to run all my tests. will give you a table of pass/fail and give you stdout/stderr and a "stack trace" for any errors
- vm.c:
  - i did way too much...
  - registers are now preallocated (its 0.65535mb it's jack shit) to avoid expensive realloc
  - bytecode functions can now (technically) be called, though i dont have full support for call and ret yet. that'll be my next MORE TARGETTED push...
  - entry frame so the entire program doesnt run on the bare vm. there is now an actual concept of a stack frame besides the struct existing in my header
  - changed EVERYTHING to use that register file. it did actually help my overhead. so many less memory operations
  - added CALL, RET, and every single fucking operation (thanks to the macro i wrote and then reused 3 times. TODO: dry that jawn)
  - inlined log_instructions to remove another function

# TODO:
  - log commits 6 thru 9
  - modularize
  - write proper tests for CALL and RET