<h2>Monorepo the stick programming language</h2>
<h3>placeholder readme. will contain syntax and allat (but that's over in the compiler for rn cuz i dont have a full compilation process finished)</h3>
<b>view: <a href="https://github.com/gamerjamer43/stickvm">StickVM</a></b><br>
<b>also: <a href="https://github.com/gamerjamer43/stickcompiler">StickCompiler</a></b>

---

**[Planned Syntax](#planned-syntax)**<br>
**[Planned Features](#planned-features)**

### Planned Syntax:
**ALSO ON THE TODO: complete the spec. this is not fully done, just some primitive features. see other repo for rn

Comments are C style. Duh.
```
// single line
/*
 * multi line
 */
```

Semicolons are OPTIONAL. You can use them to seperate statements on one line, or just add them if you want the compiler will pass it.
```
// BOTH ARE FINE
statement
statement;
```
---
Now, for the module system there's a distinction. import vs include:
```
// IMPORTED libs will already be compiled
import lib
import lib.sub

// depending on if i do path seperator, may look like rust
import std::io

// source files must be INCLUDED to be compiled along with the program
include "file.stk"

// modules wil contain source files, and additionally can contain binaries
// those will just be imported like normal
include "./module"
```
---
Declarations are still easy, but you have many options on where your vars go (option 1 and 2 are decisions for me):
```
/*
 * variables can only contain uppercase and lowercase alphabetical characters, numbers (cannot start with a number) and underscores.
 * := denotes the first time a variable is assigned.
 * now, i have to decide i want to do "let var: type :=" or "type var :=" or BOTH!
 */
// option 1, type ascription
let number: i8 := 1

// variables are by default immutable. if you want to change it denote it mutable (stolen frm rust)
let mutable number: i8 = 0;

// to send something to the constant pool, denote it const
const zero: u8 := 0

// to make something globally accessible (and send to global pool) denote global
// yeah... simple
global globular: u8 := 0

// may also add static storage. i have const and global, but if i want static too i have to define semantics
static counter: u64 := 0


// option 2, declarative style typing
i8 number := 1

const u8 zero := 0
global u8 globular := 0
static u64 counter := 0

// leaning towards 1. may allow for both
```

The only things that can be defined outside of a function scope are constant (fixed mem location fixed value), and globals (fixed mem location aka the global pool)
```
// anything outside of main scope must be constant or global
// value is constant at runtime. immutable
const i32 fuck := 42

// you can define globals outside because their memory location is fixed
// this means a lazy that is evaluated at run time is ok because we know its size at compile time
let global shit: i32 := 42
```

Only container I'm natively supporting is arrays. Everything else will be from std.containers or something:
```
// empty slots r implicitly nulled
let array: [i32, 42] := [1, 2, 3]
array[3] // is null
```

---

Control flow is prolly gonna be the same:
```
if cond { ... } else if { ... } else { ... }
while cond { ... }
do { ... } while cond
for (init, cond, inc) { ... }
for val in range { ... }

// sure there's more but i'm not sure what else
```

In typical Rust fashion*, this language will have a heavy emphasis on pattern matching.
```
// may change this one. every lang of mine has ONE weird syntax...
match case:
    |-> case1: writeln("case1 matched");
    |-> case2: writeln("case2 matched");

    // and this is why, kinda complex. might just allow for , sep
    |-> default: {
        writeln("case not matched")
    }
```
<small><b>*BUTTTT I incorporate Rust features... in C!</b></small>
---

Functions are simple. Define one with the func keyword and attach params and type.
```
// this will just be folded into a 42 wherever meaning of life is but...
func meaning_of_life () -> i64 {
    return 42;
}

func name (str name) -> str {
    // potentially making strings use String.new() for heap alloc
    str string = "Hello, " .. name!
    return string;
}
```

You can write function prototypes similar to C, and they can be hidden away with your docstrings attached
```
// will allow for prototyping in headers/interfaces
//! this is a docstring.
//! title: name
//! desc: returns a greeting with your name
//! params: name: str = your name
func name (str name) -> str
```

Main is very similar, but will always return an i32 containing 0 if successful or a panic if not (it is implicit so dw).
I may change this to work so that it returns a unit type, but uncertain of what a unit type necessarily means in my language.
```
// deciding on i32 or unit type for main return, and declarative or annotated
func main (i32 argc, str argv[]) -> i32 {
    str yoName := readln("> ")

    // standard function calls will obv be supported
    writefn("%s, dat yo name.", yoName)

    // TODO: decide how to do string formatting. other options than C style (i.e. rust style)
    writefn("{yoName}, dat yo name.")
}
```

I may also go parenthesis optional, allowing for:
```
writeln yoName + " dat yo name"
writefn "%s", yoName
```

---

Now, this language offers both structs and classes. Structs are basically 1-to-1 between C/Rust and Stick.
```
// annotated
struct Thing {
    item: i8,
}

// declarative
struct Thing {
    i8 item,
}
```

Classes though, like in python, are structs w a little overhead. In this case 16 bytes for RTTI and 16 bytes for method/field storage.
```
// annotated
class Thing {
    item: i8,

    // not sure how imma deal w borrow cemantics and ref/deref yet, so thats left out
    func set (mutable self, value: i8) -> () {
        self.i8 = value
    }
}

// declarative
class Thing {
    i8 item, 

    func set (mutable self, i8 value) -> () {
        self.i8 = value
    }
}
```

Variables can be scoped private (default, class accessible) and public (anywhere accessible)
```
// annotated
class Thing {
    pub item: i8, 

    func set (mutable self, value: i8) -> () {
        self.i8 = value
    }
}

// declarative
class Thing {
    pub i8 item, 

    func set (mutable self, i8 value) -> () {
        self.i8 = value
    }
}
```


Also... if you don't like a name I provided, sorry. But you can alias it if you really don't like it lol:
```
type uint64_t := u64;
```

---

I plan to offer referencing vs move semantics too, but as a safer concept. Not sure if I'll go thru w this
```
// referencing and deref is standard
*Thing
&Thing

// slices ofc
&[u8]
```

And generics. I'm just getting lazy w the text descriptions will mock em up it's late.
```
// may add boxed types but this is too much semantics for my high brain
class Pair<T, U> {
    public first: T
    public second: U

    func new(mutable self, first: T, second: U) {
        self.first = first;
        self.second = second;
    }

    func swap(self) -> Pair<U, T> {
        return Pair<U, T>(self.second, self.first);
    }
}

```
Also maybe native asynchronicity. Haven't thought abt this yet, may do it via standard lib to make it so Sync and Async are traits like zig.
```
// this will allow await and async to be used
Result<ValidType, ErrType>

async func test (str input) -> Result<ValidType, ErrType> {
    if input == "yes" return ValidType
    else return ErrType
}
```

Prolly will also add traits (like Rust I am borrowing a lot but it standardizes procedure, may have fallbacks but may force traits)
```
TODO
```

And also decorators (basically wrapper functions, tho i'll try and remove python's inner/outer bs).

---

And I mean that's the basics of it.

Stick supports a myriad of operators. Some of these aren't fully implemented yet:
```
Arithmetic
--------------------
+    = addition
-    = subtraction
*    = multiplication
/    = division
%    = modulus
**   = exponentiation

Assignment
--------------------
=    = assignment
+=   = add assign
-=   = subtract assign
*=   = multiply assign
/=   = divide assign
%=   = modulo assign
<<=  = left shift assign
>>=  = right shift assign
&=   = bitwise AND assign
|=   = bitwise OR assign
^=   = bitwise XOR assign

Comparison
--------------------
==   = equal
!=   = not equal
<    = less than
>    = greater than
<=   = lt or equal
>=   = gt or equal

Logical Operators
-----------------
not  = logical NOT
and  = logical AND
or   = logical OR

Bitwise Operators
-----------------
&    = bitwise AND
|    = bitwise OR
^    = bitwise XOR
~    = bitwise NOT
<<   = bitwise shift left
>>   = bitwise/arithmetic shift right (TODO: SAR)

Range / Variadic Operators
--------------------------
..   = range operator (supported: [start..] [..end] and [start..end])
...  = elipses (usage TBD)

Member / Namespace Operators (TBD)
----------------------------
.    = member access
::   = namespace or module access
->   = return type specifier (for functions)
=>   = lambda operator (TBD)
|->  = branch operator (match cases)

Other Operators (dunno yet)
---------------
?    = could be a conditional, optional, or asynchronous operator (semantics TBD).
```

And a lot of builtin types too.
```
stack allocated types*:
- numeric
  - i64 = the default signed integer type (all values are 64 bit but their width is canonical)
  - u64 = the default unsigned integer type
  - f64 = the default float type (double precision)
  - i8, u8, i16, u16, f16 (maybe), i32, u32, and f32 all supported too
  - enum (and potentially unions) will fix to its largest members size

- standard primitives
  - bool = basically a u8. legit just true or false. 0 = false, != 0 is true
  - char = also a u8. any U+256 character is ok. will be in single quotes: 'c'
  - ptr = os sized pointer (RTTI type of a pointer, either 32 or 64 bit, will likely fix this to 64 bit OSes and if anyone wants to make a 32 bit one go crazy)
  - idk what else there's gotta be more

heap allocated types**:
- strings:
    - allocate a string by just creating a double quote literal "string"
    - interning will be used, and so will slicing

- 128 bit integers/bigint
    - not implemented yet, but these will potentially be able to go on the stack in 2 consecutive registers, idk yet
    - bigint will be arbitrarily sized array of i64, doubling in size when needed
    - obv both signed and unsigned

- classes/structs
    - structs are just classes without methods and just the 16 byte RTTI header. otherwise packed properly
    - classes contain additional methods (still structs cuz C) and have a 40 byte type info.

- more. idk what else i need to add yet

**TODO:** heap + gc, heap allocated types
```
<small><b>*(trying to force anything primitive on the stack. i will say USUALLY stack allocated cuz idk conditions to move to heap.)</b></small><br>
<small><b>**(there also may be some cases where i can put these on the stack, i just dont know how yet)</b></small>

You may find a lot of the syntax similar to Go and Rust. This is because I've done a lot of reading into the design of C++, Java, Go, Rust, Lua, Python, and as you may see a wide selection of other Bytecode VMs AND fully compiled features (with the goal of making this super easily embeddable. Into what? I don't know, but I'm keeping the footprint light!)

### Planned Features:

I will also be adding an **FFI** layer that should hopefully be fully compatible with both C and Rust. Doing this for bytecode shouldn't be a problem... but once I get to the LLVM backed compiler I'm not so sure. On that we will be TBD.

I also intend to add a **JIT/runtime opt layer** that detects and optimizes hotloops while ur program is running. Additionally, it'll detect some opts the compiler may not be able to do (I don't know WHAT yet), but once we actually FINISH I can research how to write an effective JIT. I hope to get this to >65% of native speed, aiming for ~80%.