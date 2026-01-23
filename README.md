<h2>Monorepo the stick programming language</h2>
<h3>placeholder readme. will contain syntax and allat (but that's over in the compiler for rn cuz i dont have a full compilation process finished)</h3>
<b>view: <a href="https://github.com/gamerjamer43/stickvm">StickVM</a></b><br>
<b>also: <a href="https://github.com/gamerjamer43/stickcompiler">StickCompiler</a></b>

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
func name (str name) -> str;
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

- standard primitives
  - bool = basically a u8. legit just true or false. 0 = false, != 0 is true
  - char = also a u8. any U+256 character is ok. will be in single quotes: 'c'
  - idk what else there's gotta be more

heap allocated types**:
- strings:
    - allocate a string by just creating a double quote literal "string"
    - interning will be used, and so will slicing

- 


TODO: heap + gc, heap allocated types
```
<small><b>*(trying to force anything primitive on the stack. i will say USUALLY stack allocated cuz idk conditions to move to heap.)</b></small><br>
<small><b>**(there also may be some cases where i can put these on the stack, i just dont know how yet)</b></small>

You may find a lot of the syntax similar to Go and Rust. This is because I've done a lot of reading into the design of C++, Java, Go, Rust, Lua, Python, and as you may see a wide selection of other Bytecode VMs AND fully compiled features (with the goal of making this super easily embeddable. Into what? I don't know, but I'm keeping the footprint light!)