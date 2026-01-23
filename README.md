<h2>Stick Programming Language</h2>
<h3>placeholder readme</h3>
<b>view: <a href="https://github.com/gamerjamer43/stickvm">StickVM</a></b>

### Planned Syntax:
**ALSO ON THE TODO: complete the spec. this is not fully done, just some primitive features. see other repo for rn

Comments are C style. Duh.
```
// single line
/*
   multi line
*/
```

Starting off with import/vs include (an interesting hijink):
```
// libs will already be compiled (i hope)
import lib
import lib.sub
import std.io

// files have to get included to be compiled along with the program
include "file.file"
```

Declarations are too, just:
```
// deciding on := or just plain = rn but i like assignment vs reassignment
// <modifiers> <type> <name> := <val>
i32 number = 1;
const i32 zero = 0;
```
The only things that can be defined outside of a function scope are constant (fixed mem location fixed value), and globals (fixed mem location aka the global pool)
```
// anything outside of main scope must be constant or global
// value is constant at runtime. immutable
const i32 fuck = 42

// you can define globals outside because their memory location is fixed
// this means a lazy that is evaluated at run time is ok because we know its size at compile time
// thanks rust
global i32 shit = 42
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

Functions are simple. Define one with the func keyword and attach params and type.
```
func meaning_of_life () -> i64 {
    return 42;
}

func name (str name) -> str {
    // potentially making strings use String.new() for heap alloc
    str string = "Hello, " .. name!
    return string;
}
```

Control flow is prolly gonna be the same:
```
if cond { ... } else if { ... } else { ... }
while cond { ... }
do { ... } while cond
for (init, cond, inc) { ... }
for val in range { ... }

// and i cant think of anymore rn brain is slurry
```

Main is very similar, but will always return an i32 containing 0 if successful or a panic if not (it is implicit so dw).
I may change this to work so that it returns a unit type, but uncertain of what a unit type necessarily means in my language.
```
// deciding on i32 or unit type for main return
func main (i32 argc, str argv[]) -> i32 {
    // this is alr pretty go flavored so i might just keep =
    str yoName := readln("> ")
    yoName = name(yoName)

    // i may go parenthesis optional and do
    // writeln name yoName
    writeln(yoName + “ dat yo name”)
    writefn(“%s”, yoName)
}
```

I may also go parenthesis optional, allowing for:
```
writeln yoName + “ dat yo name”
writefn “%s”, yoName
```

Stick supports a myriad of operators. Here ya go pal:
```
Arithmetic
--------------------
+    = addition
-    = subtraction
*    = multiplication
/    = division
%    = modulo
**   = power/exponentiation

Assignment
--------------------
=    = assignment
+=   = add and assign
-=   = subtract and assign
*=   = multiply and assign
/=   = divide and assign
%=   = modulo and assign
<<=  = left shift and assign
>>=  = right shift and assign
&=   = bitwise AND and assign
|=   = bitwise OR and assign
^=   = bitwise XOR and assign

Comparison
--------------------
==   = equality
!=   = inequality
<    = less than
>    = greater than
<=   = less than or equal
>=   = greater than or equal

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
<<   = left shift
>>   = right shift

Range / Variadic Operators
--------------------------
..   = range operator (inclusive/exclusive TBD)
...  = variadic / spread operator (usage TBD)

Member / Namespace Operators (TBD)
----------------------------
.    = member access
::   = namespace or module access
->   = return type / arrow syntax
=>   = match arm separator
|->  = branch operator (control-flow / pipeline style)

Other Operators (dunno yet)
---------------
?    = conditional / optional operator (semantics TBD)
```

And a lot of builtin types too.
```

i64 = the default signed integer type (all values are 64 bit but their width is canonical)
u64 = the default unsigned integer type (all values are 64 bit but their width is canonical)
also: i8, u8, i16, u16, i32, u32

bool = basically a u8. legit just true or false. 0 = false, != 0 is true
char = also a u8. any U+256 character is ok

str, arr, table coming soon im done for rn
```

You may find a lot of the syntax similar to Go and Rust. This is because I've done a lot of reading into the design of C++, Java, Go, Rust, Lua, Python, and as you may see a wide selection of other Bytecode VMs AND fully compiled features (with the goal of making this super easily embeddable. Into what? I don't know, but I'm keeping the footpring light!)