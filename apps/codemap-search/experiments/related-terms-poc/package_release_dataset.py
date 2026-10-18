"""Package the pinned sense dataset without modifying it or publishing a release.

Requires the zstd executable. Output metadata is fixed; use the same zstd version
and level to reproduce the archive bytes. Only dataset.sqlite3 is archived.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import tarfile
import tempfile

DATASET_ID = "rts5-75e02862daa29308"
DATASET_SIZE = 31_715_328
DATASET_SHA256 = "90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced"
DATASET = Path(__file__).resolve().parent / "datasets" / DATASET_ID / "dataset.sqlite3"


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_archive(path):
    process = subprocess.Popen(["zstd", "-q", "-dc", str(path)], stdout=subprocess.PIPE)
    try:
        with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
            entries = iter(archive)
            entry = next(entries)
            if (entry.name, entry.isfile(), entry.size, entry.uid, entry.gid, entry.mtime, entry.mode) != (
                "dataset.sqlite3", True, DATASET_SIZE, 0, 0, 0, 0o644
            ):
                raise ValueError("archive entry metadata differs from the pinned contract")
            with archive.extractfile(entry) as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            if digest != DATASET_SHA256 or next(entries, None) is not None:
                raise ValueError("archive content differs from the pinned dataset")
        # Consume trailing compression bytes before checking the compressor's status.
        while process.stdout.read(65536):
            pass
        if process.wait() != 0:
            raise ValueError("zstd verification failed")
    finally:
        process.stdout.close()
        if process.poll() is None:
            process.terminate()
        process.wait()


def package(output):
    if DATASET.stat().st_size != DATASET_SIZE or sha256(DATASET) != DATASET_SHA256:
        raise ValueError("source size or SHA-256 differs from the pinned dataset")
    with sqlite3.connect(DATASET.as_uri() + "?mode=ro", uri=True) as connection:
        meta = dict(connection.execute("SELECT key, value FROM sense_meta"))
    if meta.get("dataset_id") != DATASET_ID or meta.get("schema_version") != "rt-sense/5":
        raise ValueError("source dataset identity differs from the pinned dataset")
    output.mkdir(parents=True, exist_ok=True)
    archive_path = output / f"{DATASET_ID}.tar.zst"
    version = subprocess.check_output(["zstd", "--version"], text=True).strip()
    with tempfile.TemporaryDirectory(prefix="rterms-package-", dir=output) as temporary:
        tar_path = Path(temporary) / "dataset.tar"
        with tarfile.open(tar_path, "w", format=tarfile.USTAR_FORMAT) as archive:
            entry = tarfile.TarInfo("dataset.sqlite3")
            entry.size = DATASET_SIZE
            entry.mode = 0o644
            entry.uid = entry.gid = entry.mtime = 0
            entry.uname = entry.gname = ""
            with DATASET.open("rb") as stream:
                archive.addfile(entry, stream)
        with archive_path.open("xb") as destination:
            subprocess.run(["zstd", "-q", "-19", "-T1", "-c", str(tar_path)], stdout=destination, check=True)
    verify_archive(archive_path)
    if sha256(DATASET) != DATASET_SHA256:
        raise ValueError("source changed while packaging")
    archive_sha256 = sha256(archive_path)
    archive_bytes = archive_path.stat().st_size
    checksum = output / (archive_path.name + ".sha256")
    checksum.write_text(f"{archive_sha256}  {archive_path.name}\n")
    notes = output / "release-notes.md"
    notes.write_text(
        f"# 관련어 데이터셋 {DATASET_ID}\n\n"
        f"- 데이터셋 id: `{DATASET_ID}`\n- 스키마: `rt-sense/5`\n"
        f"- 추출 크기: {DATASET_SIZE} bytes\n- 추출 SHA-256: `{DATASET_SHA256}`\n"
        f"- 압축 크기: {archive_bytes} bytes\n- 압축 SHA-256: `{archive_sha256}`\n\n"
        "압축 파일에는 `dataset.sqlite3`만 들어 있다. LLM이 생성한 어휘·관련어·함께 쓰는 단어에서 "
        "뜻 그래프를 만들고, 주제 매핑 검토와 저장 방식 최적화를 거친 자료다. 벡터와 생성 실행 환경은 "
        "포함하지 않는다. 데이터셋 판은 codemap-search 릴리스에 고정한다.\n"
    )
    report = {"dataset_id": DATASET_ID, "dataset_bytes": DATASET_SIZE, "dataset_sha256": DATASET_SHA256,
              "archive": str(archive_path), "archive_bytes": archive_bytes, "archive_sha256": archive_sha256,
              "checksum": str(checksum), "notes": str(notes), "zstd": version, "level": 19, "threads": 1}
    (output / "package-report.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(package(args.output_dir.resolve()), indent=2))


if __name__ == "__main__":
    main()
