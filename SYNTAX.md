## 🚨 EBNF COMIN LATER CUZ IM TOO BUSY RN 🚨

## Language Features:

### 1. import vs include

```text
import module
from module import x

include "file_to_be_compiled.sk"

// trying to decide how i should support files outside the project dir
// and the project system itself. i want to implement a build system into the compiler
```

### 2. primitive types (and variables)

```text
// definitions look similar to rust
let name: type = value

// bools are "true" or "false"
let boolean: bool = true

// integers. supports u/i 8 bit, 16 bit, 32 bit, 64 bit (128 bit support coming soon when i can figure out SSE)
let int8: i8 = 127
let uint8: u8 = 255

// floats can be single or double precision (f128 coming when the above)
let float: f32 = 1.0
let double: f64 = 1.0

// will use scalars (no surrogate codepoints allowed, so not CODEPOINTS like runes in go) like rust does with chars
// chars are single quoted
let ch: char = '🚨'

// strings are C style (null terminator will be auto added, len will subtract 1)
// strings are double quoted
let string: str = "hi there"

// any unfilled array slots get nulled by the compiler. then if you access a null value without a check it warns you
let arr: [u8, 3] = [1, 2, 3]

// (also just defining the array reserves it, and like i said above nulls all its slots)
let arr: [u8, 3]

// tagging a tuple mutable lets you replace the tuple, but not the values inside
let tuple: (u8, u8) = (1, 2)

// tables and arrays follow the same mutability structure (below)
let mut table: {u8, u8} = {
    1: 2,
    3: 4
}
```

### 3. storage specifiers

```text
// values are implicitly constant. to make a value mutable, tag it mut (this language is heavily rust flavored)
let mut seconds: u8 = 0
seconds = 100

// to ensure a values memory location does not change, tag it static
let static mut arr: [u8, 4] = [1, 2, 3, 4]
arr.resize(8) // not doable as its memory location would change

// TODO: figure out what use the global pool serves, i know how to make it just not why its there lol
```

### 4. operators

```text
let x: i32 = 3
let y: i32 = 4
let mut tot: i64 = 0

// arithmetic ops
tot = x + y
tot = x - y
tot = x * y
tot = x / y
tot = x % y // mod is %

// bitwise ops
tot = x >> y
tot = x << y
tot = x && y
tot = x || y
tot = ~y // (negate all bits of y)
// there's prolly others but

// logical ops
let boolean = not x
boolean = x and y
tot = not y

// arithmetic ops and bitwise ops also have assignment operators
tot += y

// idk where ranges logically fall but ya
let number: range(usize, usize)
```

### 5. control flow

```text
// branching is... obvious
let i: u8 = 0
if i == 0 {
    ...
} else if i < 0 {
    ...
} else {
    ...
}

// control flow is also standard to other languages
for (counter: u8 = 0, counter < 20, counter++) {
    ...
}

// condition is checked before the loop is executed
while i < 20 {
    ...
}

// executes loop once before checking the condition
do {
    ...
} while i < 20

// match cases are being worked on but look something like this
// they will be jump tabled where they can be
switch i {
    0..5 -> ...
    5 | 6 -> ...
    7 -> ...
    _ -> ...
}
```

### 6. functions

```text
// prototypes are the only line that needs to end with a semicolon
fn add(arg1: i32, arg2: i32) -> i64;

// functions are simple
fn add(arg1: i32, arg2: i32) -> i64 {
    return arg1 + arg2
}
```

### 7. structs

```text
struct Pair {
    // fields are implicitly private unless denoted pub
    key: i32,
    pub val: i32,

    fn new(self, key: i32, val: i32) -> Self {
        return Self { key, val }
    }
}

// new is both implicit and explicit using methods
let example = Pair(1, 2)
let example2 = Pair::new(3, 4)

// structs can also have default fields which must be after required fields
// to accept multiple forms of a constructor, simply overload
struct Person {
    name: str
    phone: u64
    alive: bool = true

    fn new(self, name: str, phone: u64) -> Self {
        return
    }
}

// notes:
// option 1. trying to determine a concept of protected, so potentially
// prot field3: i32
//
// option 2. get and set allows you to get or set a particular field
// if a field is pub and missing BOTH methods, they will be implicitly generated
// pub field4: i32 {
//     get() -> str {
//         return "value: {self.field4}"
//     }
// }
//
// option 0.5 (can be implemented along with one or the other)
// pub field5: i32 // has the default getter and setter
//
// trying to also decide if self should be implicit. if i provide safe pointers it shouldn't
```

### 8. traits

```text
trait VocalChords {
    fn sound(self) -> str;
}

trait Thumbs {
    fn twiddle(self) -> str;
}

// traits only provide a list of methods required
struct Hyena has VocalChords {
    fn sound(self) -> str {
        return "screeching and yelling"
    }
}

// to implement multiple traits, you need multiple implementations
struct Person has Thumbs, VocalChords {
    fn sound(self) -> str {
        return "aaaaaaaaaaaah"
    }

    fn twiddle(self) -> str {
        return "playing with my thumbs"
    }
}
```

### 9. interfaces

```text
interface Animal {
    legs: u8
    fur: bool
    wings: bool

    fn sound(self) -> str;
}

// interfaces take both the fields and methods
struct Dog is Animal {
    fn new(self) -> Self {
        Self { 4, true, false }
    }

    fn sound(self) -> str {
        return "woof"
    }
}
```

### 10. generics

```text
struct Pair[K, V] {
    key: K
    val: V

    fn new(self, key: K, val: V) -> Self {
        return Self { key, val }
    }
}

// generics require implicit typing when used as a type param, otherwise it's up to you!
let pairs: [Pair[u8, u64], 256]

// other than in the above case, these will all resolve the same by the typechecker
// (so unless you desire fully explicit types, it will be inferred just fine)
let pair = Pair(1, 2)
let pair2: Pair = Pair(3, 4)
let pair3: Pair[u8, u64] = Pair(5, 6)
```

### 11. coersion with some rules (pair example above used here):

- if the conversion is provably safe, it's allowed with no errors

```text
// coerces to i64
let integer = 1

// i am considering also allowing arbitrary bit widths in the form of a `signed` and `unsigned` type (which will shrink and grow with the number inside)
let positive: unsigned = 1
let negative: signed = -1
```

- floats will coerce to doubles by default. you have to specify if you want to use a single precision float

```text
// again, same value, different type
let single: f32 = 5.0
let double = 5.0
```

- if the coersion cant be reliably checked or is unsafe, you will receive a compiler error

```text
// examples:
// 1. signed to unsigned conversion
let value: i8 = -1
let value2: u8 = value // ERROR! THIS WOULD WRAP TO 255

// 2. float quality loss (TODO: should i do this even on numbers like 2.0?)
let double = 2.22222222222
let float: f32 = double // ERROR! this would silently lose precision

// 3. float to integer
let integer: i32 = double // ERROR! would truncate to 2 silently otherwise
```

### Planned features idk how to implement

1. pointers (issue: safety)

TODO: give an example i feel too dumb to right now
