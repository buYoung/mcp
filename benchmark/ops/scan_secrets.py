"""Count token-shaped strings in log files. Prints counts only, never the matched text.
Usage: scan_secrets.py <file or folder> [...]   (folders are walked; files over 64 MiB are skipped and listed)"""
import re
import sys
from pathlib import Path

PATTERNS = {
    'anthropic key': rb'sk-ant-[A-Za-z0-9_-]{20,}',
    'openai key': rb'sk-(?:proj-)?[A-Za-z0-9]{32,}',
    'jwt': rb'eyJ[A-Za-z0-9_-]{16,}\.eyJ[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{16,}',
    'bearer header': rb'[Bb]earer\s+[A-Za-z0-9._~+/-]{24,}',
    'github token': rb'gh[pousr]_[A-Za-z0-9]{30,}',
    'oauth token field': rb'"(?:access_token|refresh_token|id_token)"\s*:\s*"[A-Za-z0-9._~+/-]{20,}"',
    'claude oauth': rb'sk-ant-oat[0-9]{2}-[A-Za-z0-9_-]{20,}',
    'private key block': rb'-----BEGIN [A-Z ]*PRIVATE KEY-----',
}
LIMIT = 64 * 1024 * 1024

files, skipped = [], []
for arg in sys.argv[1:]:
    path = Path(arg)
    files.extend(p for p in path.rglob('*') if p.is_file() and not p.is_symlink()) if path.is_dir() else files.append(path)
totals = dict.fromkeys(PATTERNS, 0)
hits = {}
for path in files:
    if path.stat().st_size > LIMIT:
        skipped.append(str(path))
        continue
    data = path.read_bytes()
    for name, pattern in PATTERNS.items():
        count = len(re.findall(pattern, data))
        if count:
            totals[name] += count
            hits.setdefault(str(path), {})[name] = count
print('files', len(files) - len(skipped), 'skipped', len(skipped))
print('totals', totals)
for path, found in hits.items():
    print('hit', path, found)
for path in skipped:
    print('skipped', path)
