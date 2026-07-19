#!/usr/bin/env python3
"""Shared immutable-baseline constants and hashing helpers."""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
from typing import Any, Iterable


HARNESS_ROOT = pathlib.Path(__file__).resolve().parents[1]
QUALITY_ROOT = HARNESS_ROOT.parent
BENCHMARK_ROOT = QUALITY_ROOT / "benchmark"
CORPUS_ROOT = QUALITY_ROOT / "corpus/directus"
GOLDEN_CODEMAP = QUALITY_ROOT / "corpus/directus-index-golden/.codemap"
OPENCODE_BINARY = QUALITY_ROOT / "runtime/opencode"
LEGACY_ROOT = pathlib.Path("/private/tmp/codemap-search-quality.7f4a91c2")
PRODUCT_ROOT = QUALITY_ROOT.parent

MODEL = "ollama-cloud/deepseek-v4-flash"
PROVIDER = "ollama-cloud"
OPENCODE_SHA256 = "652a34cab759c0fa348f107aa737df86355a49b1576834864e89ee43c059b25d"
OPENCODE_VERSION = "1.17.18"
SOURCE_TREE_SHA256 = "e87bbfe43002f4b68c7ff9dd6218096d222daa01b0ab87f8a85525eb5becb1c0"
GOLDEN_TREE_SHA256 = "8678205ff1b19a03da85c02812aefca993b88a7688bb3db48fdf1c9b746c0a96"
ORIGINAL_PUBLIC_MANIFEST_SHA256 = "2ddb4887490025bd8272f5a14b1090a6580585539cd9bdaafedf2fa2807cb22d"
ORIGINAL_PRIVATE_MANIFEST_SHA256 = "d5445603a3d3e35d858d2e720f35e1f298e5a53dd552deab782fd253730fc4f4"
DERIVED_PUBLIC_MANIFEST_SHA256 = "edfe84b408cf24c6c58d84073176c268e5fc3e0277bd84e8bacb8095883842b7"
DERIVED_PRIVATE_MANIFEST_SHA256 = "59e0dbd42b0bc13a49590ecfddb25e91cfb320c848006e2d71b32860e643f45b"
ORIGINAL_GENERATION_FILE_SHA256 = "f061ce4cece127a6d68e250b88da916c7916542b074a47b2bf6723fea167abbf"
ORIGINAL_GENERATION_INTERNAL_SEAL = "6bf8623047f9747f613af473c654a78b58bc286005784824d3ecced1acabad1a"
ORIGINAL_PROMPT_TEMPLATE_SHA256 = "b7dcfa97aae0d852bb88c82a4009492e661b38f87339756379f9265c2600dcec"
ORIGINAL_BASELINE_1_SHA256 = "1e917c25cf8f34093652373e2c527b67419c7e6e27ab7bfd3242e6a479c17fc4"
ORIGINAL_BASELINE_2_TEMPLATE_SHA256 = "c9ce822f3bbf45b3ad1a69f5334072868ffff5a054b53e4789c0a78a4daf5a32"
ORIGINAL_LIMITS_SHA256 = "6408c39c4e89b240fb0be49983a5a5b15ba8d0618b665189a51fc673168c6bde"

TASK_IDS = (
    "API-05", "API-06", "API-01", "API-08", "API-07",
    "FE-07", "FE-06", "FE-08", "FE-03", "FE-02",
    "X-04", "X-02", "X-03", "X-06",
)
TRIAL_IDS = ("r1", "r2", "r3")
ARMS = ("B1", "B2")
QUESTION_SHA256 = {
    "API-01": "9c3fda8714bdf4ecf632451cd187e2759b40218fbebf7d8d61cb7e4daec7398f",
    "API-05": "cfb561fd55db593384dc5768af91db884f2f8a9ed3240ff4dfe49a933577d6ad",
    "API-06": "3547fa343338b255ca58089481cd4383385305225df8eecd679c1c796507f8f2",
    "API-07": "97ea73b994bfc624f38624151f886ad688c301cafbb5adb7e4684228c4d0aa7c",
    "API-08": "13bbc15b631c3256ca02b6ad457b76dee29d6754d51069151092ca7261df0af7",
    "FE-02": "25112d8c0419fbb10c0a95cf4f92e69601db59d2a50c48fa709888d0f336205e",
    "FE-03": "93044921ea6c86fb825e082deb04f4f6d8090f949c27dccdea5633dcaa4191b7",
    "FE-06": "6cce4ded5f27523434b05db6be85a4edb1311d0835c2506a114d9010780ab51c",
    "FE-07": "a058ff35e6182ec7d30efcfa13f4c9e6ac803ad10ef0e75c8ff474aa505ad58b",
    "FE-08": "2ffd63ccac0d09082d84705e95017f91ae1fc03200c2a5eab1c562da0e965429",
    "X-02": "26afee5289d892789bf0f02ac7d6bf5f92178e11a86c018428ffd216cf011ceb",
    "X-03": "2cbac74bbfabe729c8299b8aa5c4f82bedec4821d1ccc1add03fc6519bb90aad",
    "X-04": "7fca04287747002c265e9d186245d24666c7c4cf8a14858de7bb3d0297e6e3df",
    "X-06": "719a850619b3b6954fb29881675e1e4ef6800437c85c6ac15aaa5beae6ed5992",
}
ANSWER_SHA256 = {
    "API-01": "a9783f627a5191ce10e35cafd394fba56dafcf143e0d397fa685701b5a4f29b3",
    "API-05": "80d77ef85c964d16555055da4cf8a69df6e7b638ed8d50c7a1381176bbef5630",
    "API-06": "1cb4b3589854c80af8aa26d53a79d516dffa0fa6deb464b124b62e573cd13859",
    "API-07": "0d05054fdb36c27bab8e0993c35e7b2712282fa39b0e6bce5283ff7dc2b0b5a9",
    "API-08": "20157d101dae17fa60237e5a97dc4811729ccdedb8943401eb6cecc8bd26002e",
    "FE-02": "015861e767d7628c1bb64d94dd498715a25af037bae765b270ed352e2fe74c24",
    "FE-03": "408721494d714906039454e32e3967cca269123bb5cbe869891d7365fc7a02c1",
    "FE-06": "a9dd7a5a41092eef6599a08eb62b3e24fa3d4395eaa6b2cd24ede78dfdb50787",
    "FE-07": "9d8b31dd71610aa7fc75b75bb5ad0adf90292c46f65610c869a8d4075b1c99e3",
    "FE-08": "4db73417237f4b263c0c2a40542ab856b6c618e2eba3062017ecac4ca0c20b94",
    "X-02": "d6adacafc41582e1b7d5f05858c46208ca14bb86dd07aa17d5ec4d6fcb572676",
    "X-03": "874a5dd69deee6a1a0b4a7910ea87f2e36b96510668a711dd6c6f75d55cbdb84",
    "X-04": "6ad9bea5d56d9bbff3ceeadef9f140e23726665289b6504e0368e09c5d37429f",
    "X-06": "5f660c73884fab935ee85ce1282d7a81d2ac3b4dbd89cec6f157d2cc7467d24f",
}
PROMPT_SHA256 = {
    "API-01": "7c4077c6331e372749a85df51327124152b17f18c74635ab88e450c07fa1a882",
    "API-05": "598d24a14eae77829e97b4184e790cd532db7ff892aed03be362e71a74bfdc72",
    "API-06": "2cd12327bcef4207acc50d29a7e1b8f1c26718bb2fbcf0aaf6f01ce1f70e89a8",
    "API-07": "b27b9b24910c7682f2cd53a8fd3847c016884723d4044f659b325df3235a6c40",
    "API-08": "377779864f44ec12e398e81ab56b36e0e974b2218c425f80db252ff35ef77f2a",
    "FE-02": "b4b5d9002ad0705f52f2e0c50bda3d18d86fb6e8a1f2073e676038db131fbf35",
    "FE-03": "dcb54fb0444fe15ecbbf610c2b159dfb4497fa5512f0b53edb8fec53a1b1c05c",
    "FE-06": "ac1556b1baee514e7f064745af2e8ff992d09e17ba0cc9d9aba1327a8d2db8d5",
    "FE-07": "0726e7b00196edb5f1346865f2993dfde39b19d75dbf1e8a52c4652d34558e47",
    "FE-08": "e33bd06ad064543cafe9ee7a950232d22c7aae81f2c8e8d34ef1b0679a85e6a7",
    "X-02": "e371a0da80dd2ba46d5c8dc76ad27b5276f9af365aa2098b331ae85a06bb2c08",
    "X-03": "a85ee3f8ffd7b6a63ace3918de6d9f883667c5d0818a098db24d4bd7d514fffe",
    "X-04": "9404d8d3514df405fbc2990f5509477dc8146de8e530235bd8bb49ec3387e6c4",
    "X-06": "1458511eb44cf83e366d7b7a24fb538eaa37de08a0251fe65e77c108b8732b4e",
}


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_sha256(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, separators=(",", ":"), sort_keys=True).encode("utf-8")
    ).hexdigest()


def tree_digest(root: pathlib.Path, excluded_top_level: Iterable[str] = ()) -> str:
    excluded = set(excluded_top_level)
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if relative.parts and relative.parts[0] in excluded:
            continue
        digest.update(relative.as_posix().encode("utf-8") + b"\0")
        # Preserve the original generation-v4 contract: a symlink that resolves
        # to a file contributes the bytes of its target via pathlib.is_file().
        if path.is_file():
            digest.update(sha256(path).encode("ascii"))
    return digest.hexdigest()


def load_json(path: pathlib.Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: pathlib.Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    with temporary.open("w", encoding="utf-8") as stream:
        stream.write(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    try:
        descriptor = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    except OSError:
        # Some filesystems do not support directory fsync; the file itself is
        # still durable before the atomic replace.
        pass
