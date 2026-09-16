"""Address-table provenance for a bounded subset of RGBDS and x86 assembly.

The grammar parses syntax; this adapter separately follows explicit table bases
and indirect jump operands. It does not emulate instructions or execute ROMs.
"""

import re
from dataclasses import dataclass
from model import ELEMENT, MAP_ENTRIES, UNKNOWN, Fact, Location, Value, slot


@dataclass(frozen=True)
class Address:
    table: Value
    index: Value
    is_loaded: bool


def register(operand):
    """Return physical x86 register and width; narrow writes kill pointer facts."""
    operand = operand.lower().strip()
    families = {"a": "ax", "b": "bx", "c": "cx", "d": "dx", "si": "si", "di": "di", "sp": "sp", "bp": "bp"}
    for prefix, short in families.items():
        aliases = {"r" + short: 64, "e" + short: 32, short: 16}
        if len(prefix) == 1:
            aliases.update({prefix + "l": 8, prefix + "h": 8})
        else:
            aliases[prefix + "l"] = 8
        if operand in aliases:
            return "r" + short, aliases[operand]
    numbered = re.fullmatch(r"r(8|9|1[0-5])([dwb]?)", operand)
    if numbered:
        return "r" + numbered.group(1), {"": 64, "d": 32, "w": 16, "b": 8}[numbered.group(2)]
    return None


def analyze_assembly(sources):
    tables = {}
    facts = []
    for source in sources:
        label = ""
        index = 0
        for number, line in enumerate(source.data.decode("utf8").splitlines(), 1):
            code = line.split(";", 1)[0].strip()
            definition = re.match(r"^([A-Za-z_.$][\w.$]*):{1,2}(.*)$", code)
            if definition:
                label, code = definition.groups()
                index = 0
                code = code.strip()
            directive = re.match(r"(?:dw|dd|dq|\.word|\.long|\.quad)\s+(.+)$", code, re.I)
            if not label or not directive:
                continue
            for value in directive.group(1).split(","):
                value = value.strip()
                if re.fullmatch(r"[A-Za-z_.$][\w.$]*", value):
                    root = Value("global", source.path + "::" + label)
                    tables[(source.path, label)] = root
                    target = slot(slot(root, MAP_ENTRIES), Value("literal", str(index)))
                    facts.append(Fact("store", target, Value("function", source.path + "::" + value),
                                      Location(source.path, number, 1), "table:" + label,
                                      ("assembly_address_table", "linker_binding_unproven")))
                index += 1
    for source in sources:
        registers = {}
        byte_sources = {}
        hl_address = None
        pointer_bits = 64
        for number, line in enumerate(source.data.decode("utf8").splitlines(), 1):
            code = line.split(";", 1)[0].strip()
            if not code:
                continue
            if re.match(r"^[A-Za-z_.$][\w.$]*:", code):
                registers.clear()
                byte_sources.clear()
                hl_address = None
                continue
            mode = re.fullmatch(r"bits\s+(32|64)", code, re.I)
            if mode:
                pointer_bits = int(mode.group(1))
                registers.clear()
                continue
            instruction = re.match(r"(\w+)\s*(.*)", code)
            if not instruction:
                continue
            operation, arguments = instruction.groups()
            operation = operation.lower()
            operands = [part.strip() for part in arguments.split(",")]
            destination = operands[0].lower()
            if operation in {"ld", "lea", "mov"} and len(operands) == 2:
                raw = operands[1].strip()
                operand = re.sub(r"\b(?:rel|qword|dword|ptr)\b|[\[\]]", " ", raw, flags=re.I).strip()
                key = (source.path, operand)
                candidates = [value for (path, name), value in tables.items() if name == operand]
                table = tables.get(key) or (candidates[0] if len(candidates) == 1 else None)
                if operation == "ld":
                    if destination == "hl":
                        hl_address = table if "[" not in raw else None
                        byte_sources.pop("h", None)
                        byte_sources.pop("l", None)
                    elif destination in {"a", "h", "l"}:
                        if destination == "a" and raw.lower() == "[hli]" and hl_address is not None:
                            byte_sources["a"] = (hl_address, "low")
                        elif destination == "h" and raw.lower() == "[hl]" and hl_address is not None:
                            byte_sources["h"] = (hl_address, "high")
                        elif raw.lower() in byte_sources:
                            byte_sources[destination] = byte_sources[raw.lower()]
                        else:
                            byte_sources.pop(destination, None)
                        if destination in {"h", "l"}:
                            hl_address = None
                    continue
                target_register = register(destination)
                if target_register:
                    prior = dict(registers)
                    canonical, width = target_register
                    registers.pop(canonical, None)
                    if width == pointer_bits:
                        if table is not None:
                            registers[canonical] = Address(table, Value("literal", "0"), operation == "mov" and "[" in raw)
                        else:
                            input_register = register(operand)
                            if input_register and input_register[1] == pointer_bits and input_register[0] in prior:
                                origin = prior[input_register[0]]
                                if "[" not in raw and operation == "mov":
                                    registers[canonical] = origin
                                elif "[" in raw and operation == "mov" and not origin.is_loaded:
                                    registers[canonical] = Address(origin.table, origin.index, True)
                continue
            if operation in {"jp", "jmp", "call"}:
                address = None
                if destination == "hl":
                    low, high = byte_sources.get("l"), byte_sources.get("h")
                    if low and high and low[0] == high[0] and low[1] == "low" and high[1] == "high":
                        address = Address(low[0], Value("dynamic_key", source.path + ":index:" + str(number)), True)
                called = register(destination.lstrip("*"))
                if called and called[1] == pointer_bits:
                    address = registers.get(called[0])
                if address is not None and address.is_loaded:
                    target = slot(slot(address.table, MAP_ENTRIES), address.index)
                    facts.append(Fact("invoke", target, UNKNOWN, Location(source.path, number, 1),
                                      "indirect:" + source.path,
                                      ("assembly_control_flow_unproven", "table_entry_index_required", "register_provenance_subset")))
                registers.clear()
                byte_sources.clear()
                hl_address = None
            elif operation == "add" and destination == "hl" and len(operands) == 2 and operands[1].lower() in {"bc", "de"}:
                # Preserve the RGBDS table base while leaving its index unknown.
                pass
            elif operation.startswith("ret") or operation.startswith("j"):
                registers.clear()
                byte_sources.clear()
                hl_address = None
            elif operation not in {"cmp", "test", "push", "nop", "bits", "section"}:
                written = register(destination)
                if operation not in {"xor", "sub", "add", "and", "or", "pop", "inc", "dec", "shl", "shr", "sal", "sar", "not", "neg"}:
                    # Unknown instructions may have multiple/implicit outputs
                    # (e.g. xchg, mul); retaining another register is unsafe.
                    registers.clear()
                elif written:
                    registers.pop(written[0], None)
                if destination in {"a", "h", "l"}:
                    byte_sources.pop(destination, None)
                if destination in {"h", "l", "hl"}:
                    hl_address = None
    return facts
