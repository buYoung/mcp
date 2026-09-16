"""Byte-aligned, type-only fallback views for unsupported TypeScript syntax."""

import re


def blank_range(output, start, end):
    for index in range(start, end):
        if output[index] not in {10, 13}:
            output[index] = 32


def code_only(data):
    """Hide comments and quoted text for locating actual declarations."""
    output = bytearray(data)
    index = 0
    while index < len(data):
        start = index
        if data[index:index + 2] == b"/*":
            end = data.find(b"*/", index + 2)
            index = len(data) if end < 0 else end + 2
        elif data[index:index + 2] == b"//":
            end = data.find(b"\n", index + 2)
            index = len(data) if end < 0 else end
        elif data[index] in {34, 39, 96}:
            quote = data[index]
            index += 1
            while index < len(data):
                if data[index] == 92:
                    index += 2
                    continue
                if data[index] == quote:
                    index += 1
                    break
                index += 1
        else:
            index += 1
            continue
        blank_range(output, start, min(index, len(data)))
    return bytes(output)


def project_type_parameter_modifiers(data):
    pattern = rb"([<,]\s*)((?:in\s+out|out|in|const)\s+)(?=[A-Za-z_$][\w$]*(?:\s*[,>:=]|\s+extends\b))"
    output = bytearray(data)
    for match in re.finditer(pattern, code_only(data)):
        blank_range(output, match.start(2), match.end(2))
    return bytes(output)


def project_type_object_bodies(data):
    """Erase declaration type bodies, preserving runtime object literals."""
    code = code_only(data)
    output = bytearray(project_type_parameter_modifiers(data))
    starts = []
    declaration = rb"(?m)^[\t ]*(?:export[\t ]+)?(?:declare[\t ]+)?"
    for pattern in (declaration + rb"(?:const|let|var)\s+[A-Za-z_$][\w$]*\s*:\s*\{",
                    declaration + rb"type\s+[A-Za-z_$][\w$]*\s*=\s*\{"):
        starts.extend(match.end() - 1 for match in re.finditer(pattern, code))
    for match in re.finditer(declaration + rb"interface\s+[A-Za-z_$][\w$]*", code):
        depth = 0
        index = match.end()
        while index < len(code):
            byte = code[index]
            if byte == 60:
                depth += 1
            elif byte == 62 and code[index - 1] != 61:
                depth = max(0, depth - 1)
            elif byte == 123 and depth == 0:
                starts.append(index)
                break
            elif byte in {59, 61} and depth == 0:
                break
            index += 1
    for start in starts:
        depth = 1
        index = start + 1
        while index < len(code) and depth:
            if code[index] == 123:
                depth += 1
            elif code[index] == 125:
                depth -= 1
            index += 1
        if depth == 0:
            blank_range(output, start + 1, index - 1)
    return bytes(output)
