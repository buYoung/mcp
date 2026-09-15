"""Address-table provenance for a bounded subset of RGBDS and x86 assembly.

The grammar parses syntax; this adapter separately follows explicit table bases
and indirect jump operands. It does not emulate instructions or execute ROMs.
"""

import re
from model import ELEMENT, MAP_ENTRIES, UNKNOWN, Fact, Location, Value, slot


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
        base = None
        has_low_byte = False
        has_high_byte = False
        for number, line in enumerate(source.data.decode("utf8").splitlines(), 1):
            code = line.split(";", 1)[0].strip()
            if not code:
                continue
            if re.match(r"^[A-Za-z_.$][\w.$]*:", code):
                base, has_low_byte, has_high_byte = None, False, False
                continue
            load = re.match(r"(?:ld|lea|mov)\s+(hl|rax|eax|r\d+),\s*(.*)$", code, re.I)
            if load:
                operand = re.sub(r"\b(?:rel|rip|qword|dword|ptr)\b|[\[\]+]", " ", load.group(2)).strip()
                key = (source.path, operand)
                candidates = [value for (path, name), value in tables.items() if name == operand]
                base = tables.get(key) or (candidates[0] if len(candidates) == 1 else None)
                has_low_byte = has_high_byte = "[" in load.group(2) and load.group(1).lower() != "hl"
                continue
            if re.match(r"ld\s+a,\s*\[hli\]", code, re.I):
                has_low_byte = base is not None
            elif re.match(r"ld\s+h,\s*\[hl\]", code, re.I):
                has_high_byte = base is not None
            elif re.match(r"ld\s+hl,", code, re.I):
                base = None
            elif re.match(r"(?:jp|jmp|call)\s+\*?(?:hl|rax|eax|r\d+)\s*$", code, re.I):
                if base is not None and has_low_byte and has_high_byte:
                    target = slot(slot(base, MAP_ENTRIES), Value("dynamic_key", source.path + ":index:" + str(number)))
                    facts.append(Fact("invoke", target, UNKNOWN, Location(source.path, number, 1),
                                      "indirect:" + source.path,
                                      ("assembly_control_flow_unproven", "table_entry_index_required", "register_provenance_subset")))
                base = None
            elif re.match(r"ret\b", code, re.I):
                base = None
    return facts
