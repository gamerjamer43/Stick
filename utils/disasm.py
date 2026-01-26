"""
Simple disassembler for .stk bytecode files produced by `test.py`.
Usage: python disasm.py path/to/file.stk
"""
from sys import argv
from struct import unpack
from pathlib import Path
from .test import Opcode, Type

# easy maps for easy lookup (and saving space lmao)
OPCODES = {o.value: o.name for o in Opcode}
TYPES = {t.value: t.name for t in Type}

def signed(val: int, bits: int) -> int:
    """decode a signed integer"""
    mask = 1 << (bits - 1)

    # if sign bit is 1 val & mask = mask (nonzero)
    if val & mask: return val - (1 << bits)
    return val

def decode_instr(word: int) -> tuple[str, tuple[int, int, int]]:
    """decode an instruction"""
    op = (word >> 24) & 0xFF
    a = (word >> 16) & 0xFF
    b = (word >> 8) & 0xFF
    c = word & 0xFF

    name = OPCODES.get(op, f"OP_{op}")
    return name, (a, b, c)

def format_instr(idx: int, word: int) -> str:
    """format instruction into something human readable"""
    # get both properly formatted and raw instruction
    name, (a, b, c) = decode_instr(word)
    raw = f"0x{word:08X}"

    if name == "HALT":
        return f"{idx:04d}: {raw}  HALT"
    
    if name == "PANIC":
        return f"{idx:04d}: {raw}  PANIC code={a}"
    
    # register movement
    if name == "LOADI":
        imm = (b << 8) | c
        imm = signed(imm, 16)

        return f"{idx:04d}: {raw}  LOADI r{a}, {imm}"
    
    if name in ("LOADC", "LOADG", "STOREG"):
        return f"{idx:04d}: {raw}  {name} r{a}, idx={b}"
    
    if name in ("COPY", "MOVE"):
        return f"{idx:04d}: {raw}  {name} r{a}, r{b}"
    
    # jumps
    if name == "JMP":
        off = (a << 16) | (b << 8) | c
        off = signed(off, 24)
        return f"{idx:04d}: {raw}  JMP {off:+d}"
    
    if name in ("JMPIF", "JMPIFZ"):
        return f"{idx:04d}: {raw}  {name} r{a}, off={c}"
    
    # all binary ops
    if name in (
        "AND","OR","XOR","SHL","SHR","SAR",
        "I2D","I2F","D2I","F2I","I2U","U2I","U2D","U2F","D2U","F2U",
        "ADD","SUB","MUL","DIV","MOD","EQ","NEQ","GT","GE","LT","LE",
        "ADD_U","SUB_U","MUL_U","DIV_U","MOD_U","EQ_U","NEQ_U","GT_U","GE_U","LT_U","LE_U",
        "ADD_F","SUB_F","MUL_F","DIV_F","EQ_F","NEQ_F","GT_F","GE_F","LT_F","LE_F",
        "ADD_D","SUB_D","MUL_D","DIV_D","EQ_D","NEQ_D","GT_D","GE_D","LT_D","LE_D",
        "AND_U","OR_U","XOR_U","SHL_U","SHR_U",
    ):
        return f"{idx:04d}: {raw}  {name} r{a}, r{b}, r{c}"
    
    # unary ops
    if name.startswith("NEG") or name in ("LNOT","BNOT","BNOT_U","NEG_U"):
        return f"{idx:04d}: {raw}  {name} r{a}"
    
    # fallback
    return f"{idx:04d}: {raw}  {name} a={a} b={b} c={c}"

# decode something in a constant slot
def decode_const(data: bytes) -> str:
    # must be 9 bytes (type byte and payload)
    if len(data) != 9: return f"<bad slot len={len(data)}>"
    
    # the actual decoding
    type, body = data[0], data[1:]
    name = TYPES.get(type, f"TYPE_{type}")
    val = body.hex()

    # null can just escape
    if name == "NUL": return "NUL"

    # anything non zero is true
    if name == "BOOL":
        val = unpack("<q", body)[0]
        return f"BOOL {bool(val)}"
    
    # numeric
    if name == "U64": val = unpack("<Q", body)[0]
    if name == "I64": val = unpack("<q", body)[0]
    if name == "FLOAT": val = unpack("<f", body[:4])[0]
    if name == "DOUBLE": val = unpack("<d", body)[0]
    
    # functions have extra fluff
    if name == "CALLABLE":
        entry, argc, regc = unpack("<IHH", body)
        return f"CALLABLE entry={entry} argc={argc} regc={regc}"
    
    return f"{name} {val}"

# run a full disassembly and print results
def disassemble(path: Path) -> None:
    with path.open("rb") as f:
        hdr = f.read(4)
        if len(hdr) < 4:
            return print("file too short")

        version, flags, n_ins, n_consts, n_globs = unpack("<HHIII", f.read(16))
        print(f"file: {path}")
        print(f"header: magic={hdr!r} version={version} flags={flags} instrs={n_ins} consts={n_consts} globs={n_globs}")

        words = list(unpack(f"<{n_ins}I", f.read(n_ins * 4))) if n_ins else []

        print("\nInstructions:")
        for i, w in enumerate(words):
            print(format_instr(i, w))

        print("\nConstants:")
        for i in range(n_consts):
            slot = f.read(9)
            print(f"  [{i}] {decode_const(slot)}")

        print("\nGlobals:")
        for i in range(n_globs):
            slot = f.read(9)
            print(f"  [{i}] {decode_const(slot)}")


def main() -> None:
    if len(argv) < 2:
        return print("usage: disasm.py file.stk")
    
    args = argv[1:]
    for p in args: disassemble(Path(p))

if __name__ == "__main__":
    main()