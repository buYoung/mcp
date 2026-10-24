"""Durable campaign snapshots and the shared prepare/run resource lease."""
from __future__ import annotations

import contextlib
import datetime
import os
import tempfile
from pathlib import Path

from . import resources, route_contract as routes
from .core import canonical, file_digest, require


def timestamp() -> str:
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def durable_bytes(path: Path, data: bytes, *, exclusive=False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        if exclusive:
            stream = path.open("xb")
        else:
            fd, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
            temporary = Path(name)
            stream = os.fdopen(fd, "wb")
        with stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        if temporary:
            temporary.replace(path)
        fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    finally:
        if temporary:
            temporary.unlink(missing_ok=True)


def durable_json(path: Path, value, *, exclusive=False) -> None:
    durable_bytes(path, (canonical(value) + "\n").encode(), exclusive=exclusive)


def immutable_copy(source: Path, target: Path) -> str:
    checksum = file_digest(source)
    if target.exists():
        require(file_digest(target) == checksum, f"불변 근거 덮어쓰기 금지: {target}")
    else:
        durable_bytes(target, source.read_bytes(), exclusive=True)
    return checksum


@contextlib.contextmanager
def campaign_lease():
    # One host-wide lease, not one per cache/model. No probing or killing port owners.
    host = Path("/private/tmp" if Path("/private/tmp").is_dir() else "/tmp")
    with resources.lease(host / f"codemap-bench-ports-{os.getuid()}.lock"):
        with resources.lease(routes.PREPARATION / ".prepare.lock"):
            yield
