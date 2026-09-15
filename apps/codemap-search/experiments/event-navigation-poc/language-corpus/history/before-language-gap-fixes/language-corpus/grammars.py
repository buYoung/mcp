"""Load pinned grammars; compile only the repository's vendored Groovy parser."""

import argparse
import ctypes
import gzip
import hashlib
from pathlib import Path
import subprocess
import tempfile

from tree_sitter import Language, Parser
from tree_sitter_language_pack import get_parser

DIRECTORY = Path(__file__).resolve().parent
PACKAGE = DIRECTORY.parents[2]
VENDOR = PACKAGE / "vendor/tree-sitter-groovy/src"
LIBRARY = DIRECTORY / "_native/groovy.so"
_libraries = []


def fingerprint():
    return {str(path.relative_to(VENDOR)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in [VENDOR / "parser.c.gz", *sorted((VENDOR / "tree_sitter").glob("*.h"))]}


def build_groovy():
    import json
    LIBRARY.parent.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="event-poc-grammar-") as directory:
        parser = Path(directory) / "parser.c"
        parser.write_bytes(gzip.decompress((VENDOR / "parser.c.gz").read_bytes()))
        output = Path(directory) / "groovy.so"
        subprocess.run(["cc", "-shared", "-fPIC", "-O2", "-std=c11", "-I", str(VENDOR),
                        str(parser), "-o", str(output)], check=True)
        LIBRARY.write_bytes(output.read_bytes())
    (LIBRARY.parent / "groovy-inputs.json").write_text(json.dumps(fingerprint(), indent=2) + "\n")


def parser_for(language):
    if language != "groovy":
        return get_parser("asm" if language == "assembly" else language)
    import json
    if not LIBRARY.is_file():
        raise RuntimeError("Groovy parser is not built. Run: python grammars.py --build")
    if json.loads((LIBRARY.parent / "groovy-inputs.json").read_text()) != fingerprint():
        raise RuntimeError("Vendored Groovy grammar changed; rebuild it with grammars.py --build")
    library = ctypes.CDLL(str(LIBRARY))
    library.tree_sitter_groovy.restype = ctypes.c_void_p
    capsule_new = ctypes.pythonapi.PyCapsule_New
    capsule_new.restype = ctypes.py_object
    capsule_new.argtypes = (ctypes.c_void_p, ctypes.c_char_p, ctypes.c_void_p)
    capsule = capsule_new(library.tree_sitter_groovy(), b"tree_sitter.Language", None)
    _libraries.append(library)
    return Parser(Language(capsule))


if __name__ == "__main__":
    options = argparse.ArgumentParser(description=__doc__)
    options.add_argument("--build", action="store_true", required=True)
    options.parse_args()
    build_groovy()
    print(LIBRARY)
