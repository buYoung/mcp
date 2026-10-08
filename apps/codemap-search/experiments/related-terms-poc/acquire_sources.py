#!/usr/bin/env python3
"""Freeze and verify the upstream sources of the related-terms dataset (offline only).

Network retrieval is performed by the explicit `curl`/`git` commands listed in
README.ko.md; this script never opens a network connection.

    python3 acquire_sources.py plan-wikidata     # write wikidata/urls.txt + requests.json
    python3 acquire_sources.py record-wikidata   # validate and hash the curl responses
    python3 acquire_sources.py pack              # deterministic archives + sources.lock.json
    python3 acquire_sources.py verify            # re-hash archives against the lock
    python3 acquire_sources.py plan-revision-audit --sample 20     # wikidata/audit-urls.txt
    python3 acquire_sources.py compare-revision-audit              # compare curl responses

Raw downloads and archives live under the ignored `.codemap/poc/related-terms/sources/`.
Only `sources.lock.json` is tracked.
"""

from __future__ import annotations

import argparse
import io
import json
import random
import subprocess
import sys
import tarfile
import urllib.parse
from pathlib import Path

from rtpoc_common import (
    ARCHIVE_DIR,
    RAW_DIR,
    SOURCES_LOCK_PATH,
    parse_front_matter,
    read_json,
    repo_relative,
    sha256_bytes,
    sha256_file,
    write_json,
)

WIKIDATA_API = "https://www.wikidata.org/w/api.php"
WIKIDATA_BATCH_SIZE = 50
# Entities examined while the brief was authored. They are diagnostic controls, not a
# random sample; child 03 must label any evaluation case derived from them as a control.
WIKIDATA_DIAGNOSTIC_IDS = ["Q1127410", "Q1349935", "Q1376168"]

WORDNET = {
    "source_id": "oewn-2025",
    "repository": "https://github.com/globalwordnet/english-wordnet",
    "tag": "2025-edition",
    "commit": "dc343f2683279ecbb13fab4e2fd778d7b162d287",
    "asset_url": "https://github.com/globalwordnet/english-wordnet/releases/download/2025-edition/english-wordnet-2025-json.zip",
    "asset_sha256": "7d749f6e2c39e6970e4997839dcf6e42fd281f3c2fae0171d2192bae8cfa4b51",
    "license_files": ["LICENSE.md", "WNDB_License.txt"],
}

GIT_SOURCES = {
    "github-explore": {
        "source_id": "github-explore-c8d7bc5",
        "repository": "https://github.com/github/explore",
        "commit": "c8d7bc516928a4f17845b767884a80cc53affd57",
        "raw_dir": "explore",
        "sparse": ["/topics/*/index.md", "/LICENSE.txt", "/notices.md"],
        "include": lambda path: (path.startswith("topics/") and path.endswith("/index.md") and path.count("/") == 2)
        or path in {"LICENSE.txt", "notices.md"},
        "license_files": ["LICENSE.txt", "notices.md"],
    },
    "abbrcode": {
        "source_id": "abbrcode-bd33b1c",
        "repository": "https://github.com/abbrcode/abbreviations-in-code",
        "commit": "bd33b1c7bbfea159275c88d223482f5da9a01e58",
        "raw_dir": "abbrcode",
        "sparse": None,
        "include": lambda path: path in {"data/abbrs/.json", "LICENCE", "README.md"},
        "license_files": ["LICENCE"],
    },
    "se-abbreviations": {
        "source_id": "se-abbr-8e1d77e",
        "repository": "https://github.com/wuyichen24/software-engineering-abbreviations-acronyms",
        "commit": "8e1d77e99b184106d50a180ba52cb3557273af6d",
        "raw_dir": "se-abbr",
        "sparse": None,
        "include": lambda path: (path.startswith("dictionary/") and path.endswith(".md")) or path in {"LICENSE", "README.md"},
        "license_files": ["LICENSE"],
    },
}


def run(command: list[str], cwd: Path | None = None) -> str:
    result = subprocess.run(command, cwd=cwd, check=True, capture_output=True, text=True)
    return result.stdout


def explore_wikipedia_titles() -> tuple[list[str], list[dict]]:
    """English Wikipedia titles named by GitHub Explore topics at the pinned commit."""
    spec = GIT_SOURCES["github-explore"]
    target = RAW_DIR / spec["raw_dir"]
    titles: set[str] = set()
    exclusions: list[dict] = []
    for path in git_files(target, spec):
        if not path.startswith("topics/"):
            continue
        topic = path.split("/")[1]
        fields = parse_front_matter(git_blob(target, spec["commit"], path).decode("utf-8"))
        url = fields.get("wikipedia_url")
        if not url:
            continue
        parts = urllib.parse.urlsplit(url)
        if parts.netloc != "en.wikipedia.org" or not parts.path.startswith("/wiki/"):
            exclusions.append({"topic": topic, "wikipedia_url": url, "reason": "not_english_wikipedia_article"})
            continue
        if parts.fragment:
            exclusions.append({"topic": topic, "wikipedia_url": url, "reason": "section_link_not_article_concept"})
            continue
        titles.add(urllib.parse.unquote(parts.path[len("/wiki/") :]).replace("_", " "))
    return sorted(titles), exclusions


def plan_wikidata() -> None:
    """Write the exact wbgetentities requests; `curl` performs them so each URL is reviewable."""
    target = RAW_DIR / "wikidata"
    (target / "responses").mkdir(parents=True, exist_ok=True)
    titles, exclusions = explore_wikipedia_titles()
    requests: list[dict] = []
    base = {
        "action": "wbgetentities",
        "props": "info|labels|aliases|descriptions|sitelinks",
        "languages": "en",
        "sitefilter": "enwiki",
        "format": "json",
    }
    batches = [("titles", titles[i : i + WIKIDATA_BATCH_SIZE]) for i in range(0, len(titles), WIKIDATA_BATCH_SIZE)]
    batches.append(("ids", WIKIDATA_DIAGNOSTIC_IDS))
    for index, (kind, values) in enumerate(batches):
        params = dict(base)
        if kind == "titles":
            params.update({"sites": "enwiki", "titles": "|".join(values)})
        else:
            params.update({"ids": "|".join(values)})
        name = f"{kind}-{index:03d}.json"
        url = WIKIDATA_API + "?" + urllib.parse.urlencode(params)
        requests.append({"file": f"responses/{name}", "params": params, "url": url})
    (target / "urls.txt").write_text(
        "".join(f"{request['file']} {request['url']}\n" for request in requests), encoding="utf-8"
    )
    write_json(
        target / "requests.json",
        {
            "api": WIKIDATA_API,
            "selection_policy": "W1: entities whose enwiki sitelink equals a GitHub Explore topic wikipedia_url "
            "(article links only); W2: authoring diagnostic entities",
            "diagnostic_ids": WIKIDATA_DIAGNOSTIC_IDS,
            "title_count": len(titles),
            "url_exclusions": exclusions,
            "requests": requests,
        },
    )


def unresolved_titles(plan: dict) -> list[str]:
    target = RAW_DIR / "wikidata"
    missing: set[str] = set()
    for request in plan["requests"]:
        for entity in json.loads((target / request["file"]).read_bytes()).get("entities", {}).values():
            if "missing" in entity and entity.get("title"):
                missing.add(entity["title"])
    return sorted(missing)


def plan_wikidata_redirects() -> None:
    """Stage 2a: ask English Wikipedia which unresolved titles are page redirects (titles only)."""
    target = RAW_DIR / "wikidata"
    plan = read_json(target / "requests.json")
    titles = unresolved_titles(plan)
    redirect_requests = []
    for index in range(0, len(titles), WIKIDATA_BATCH_SIZE):
        params = {"action": "query", "redirects": "1", "format": "json", "titles": "|".join(titles[index : index + WIKIDATA_BATCH_SIZE])}
        url = "https://en.wikipedia.org/w/api.php?" + urllib.parse.urlencode(params)
        redirect_requests.append({"file": f"responses/redirects-{index // WIKIDATA_BATCH_SIZE:03d}.json", "params": params, "url": url})
    plan["redirect_requests"] = redirect_requests
    write_json(target / "requests.json", plan)
    (target / "redirect-urls.txt").write_text(
        "".join(f"{request['file']} {request['url']}\n" for request in redirect_requests), encoding="utf-8"
    )


def redirect_targets(plan: dict) -> dict[str, str]:
    """Map each unresolved title through Wikipedia's normalization and redirect hops."""
    target = RAW_DIR / "wikidata"
    mapping: dict[str, str] = {}
    for request in plan.get("redirect_requests", []):
        query = json.loads((target / request["file"]).read_bytes()).get("query", {})
        # A redirect into a section names a narrower or different concept than the article.
        hops = {
            item["from"]: item["to"]
            for item in query.get("normalized", []) + query.get("redirects", [])
            if "tofragment" not in item
        }
        for title in request["params"]["titles"].split("|"):
            current, seen = title, set()
            while current in hops and current not in seen:
                seen.add(current)
                current = hops[current]
            if current != title:
                mapping[title] = current
    return mapping


def plan_wikidata_resolved() -> None:
    """Stage 2b: wbgetentities for redirect targets that stage 1 did not already resolve."""
    target = RAW_DIR / "wikidata"
    plan = read_json(target / "requests.json")
    plan["requests"] = [request for request in plan["requests"] if not request["file"].startswith("responses/resolved-")]
    mapping = redirect_targets(plan)
    titles = sorted(set(mapping.values()))
    base = {k: v for k, v in plan["requests"][0]["params"].items() if k not in ("titles", "ids")}
    resolved_requests = []
    for index in range(0, len(titles), WIKIDATA_BATCH_SIZE):
        params = dict(base)
        params.update({"sites": "enwiki", "titles": "|".join(titles[index : index + WIKIDATA_BATCH_SIZE])})
        url = WIKIDATA_API + "?" + urllib.parse.urlencode(params)
        resolved_requests.append({"file": f"responses/resolved-{index // WIKIDATA_BATCH_SIZE:03d}.json", "params": params, "url": url})
    plan["requests"] += resolved_requests
    plan["redirect_mapping"] = dict(sorted(mapping.items()))
    write_json(target / "requests.json", plan)
    (target / "resolved-urls.txt").write_text(
        "".join(f"{request['file']} {request['url']}\n" for request in resolved_requests), encoding="utf-8"
    )


def record_wikidata() -> None:
    """Validate and hash every fetched Wikidata/Wikipedia response named by requests.json."""
    target = RAW_DIR / "wikidata"
    plan = read_json(target / "requests.json")
    for request in plan.get("redirect_requests", []):
        body = (target / request["file"]).read_bytes()
        if "error" in json.loads(body):
            raise SystemExit(f"redirect request failed: {request['file']}")
        request["sha256"] = sha256_bytes(body)
    for request in plan["requests"]:
        body = (target / request["file"]).read_bytes()
        parsed = json.loads(body)
        if "error" in parsed or parsed.get("success") != 1:
            raise SystemExit(f"wikidata request failed: {request['file']}: {parsed.get('error')}")
        request["sha256"] = sha256_bytes(body)
    write_json(target / "requests.json", plan)


# --- pack --------------------------------------------------------------------------------


def git_files(target: Path, spec: dict) -> list[str]:
    listing = run(["git", "ls-tree", "-r", "--name-only", spec["commit"]], cwd=target).splitlines()
    return sorted(path for path in listing if spec["include"](path))


def git_blob(target: Path, commit: str, path: str) -> bytes:
    return subprocess.run(
        ["git", "cat-file", "blob", f"{commit}:{path}"], cwd=target, check=True, capture_output=True
    ).stdout


def git_blob_id(target: Path, commit: str, path: str) -> str:
    return run(["git", "rev-parse", f"{commit}:{path}"], cwd=target).strip()


def write_deterministic_tar(path: Path, members: list[tuple[str, bytes]]) -> None:
    """Uncompressed tar with sorted names and fixed metadata, so equal inputs hash equally."""
    path.parent.mkdir(parents=True, exist_ok=True)
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, data in sorted(members):
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mtime = 0
            info.mode = 0o644
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            archive.addfile(info, io.BytesIO(data))
    path.write_bytes(buffer.getvalue())


def pack_git_source(key: str) -> dict:
    spec = GIT_SOURCES[key]
    target = RAW_DIR / spec["raw_dir"]
    files = git_files(target, spec)
    manifest = []
    members = []
    for path in files:
        data = git_blob(target, spec["commit"], path)
        members.append((f"{spec['source_id']}/{path}", data))
        manifest.append({"path": path, "git_blob": git_blob_id(target, spec["commit"], path), "bytes": len(data)})
    members.append(
        (f"{spec['source_id']}/_manifest.json", (json.dumps(manifest, indent=1, sort_keys=True) + "\n").encode())
    )
    archive = ARCHIVE_DIR / f"{spec['source_id']}.tar"
    write_deterministic_tar(archive, members)
    tree = run(["git", "rev-parse", f"{spec['commit']}^{{tree}}"], cwd=target).strip()
    return {
        "source_id": spec["source_id"],
        "repository": spec["repository"],
        "revision": {"commit": spec["commit"], "tree": tree},
        "archive_path": repo_relative(archive),
        "archive_sha256": sha256_file(archive),
        "archive_bytes": archive.stat().st_size,
        "archived_files": len(files),
        "license_files": spec["license_files"],
        "retrieval": (
            f"git clone --filter=blob:none --no-checkout {spec['repository']}.git && "
            f"git sparse-checkout set --no-cone {' '.join(spec['sparse'])} && "
            if spec["sparse"]
            else f"git clone {spec['repository']}.git && "
        )
        + f"git checkout {spec['commit']}; `python3 acquire_sources.py pack` reads the pinned blobs",
    }


def pack_wordnet() -> dict:
    target = RAW_DIR / "wordnet"
    members = [(f"oewn-2025/{name}", (target / name).read_bytes()) for name in WORDNET["license_files"]]
    license_archive = ARCHIVE_DIR / "oewn-2025-license.tar"
    write_deterministic_tar(license_archive, members)
    asset = target / "english-wordnet-2025-json.zip"
    return {
        "source_id": WORDNET["source_id"],
        "repository": WORDNET["repository"],
        "revision": {"tag": WORDNET["tag"], "commit": WORDNET["commit"]},
        "archive_path": repo_relative(asset),
        "archive_sha256": sha256_file(asset),
        "archive_bytes": asset.stat().st_size,
        "license_archive_path": repo_relative(license_archive),
        "license_archive_sha256": sha256_file(license_archive),
        "license_files": WORDNET["license_files"],
        "retrieval": f"curl -L -o english-wordnet-2025-json.zip {WORDNET['asset_url']} "
        f"(release asset; license files from raw.githubusercontent.com at {WORDNET['commit']})",
    }


def pack_wikidata() -> dict:
    target = RAW_DIR / "wikidata"
    requests = read_json(target / "requests.json")
    members = [("wikidata-sel1/requests.json", (target / "requests.json").read_bytes())]
    revisions: dict[str, int] = {}
    for request in requests.get("redirect_requests", []) + requests["requests"]:
        body = (target / request["file"]).read_bytes()
        if sha256_bytes(body) != request["sha256"]:
            raise SystemExit(f"wikidata response changed on disk: {request['file']}")
        members.append((f"wikidata-sel1/{request['file']}", body))
        for entity_id, entity in json.loads(body).get("entities", {}).items():
            if "missing" not in entity:
                revisions[entity_id] = entity["lastrevid"]
    archive = ARCHIVE_DIR / "wikidata-sel1.tar"
    write_deterministic_tar(archive, members)
    return {
        "source_id": "wikidata-sel1",
        "repository": "https://www.wikidata.org",
        "revision": {
            "entity_revisions_sha256": sha256_bytes(json.dumps(revisions, sort_keys=True).encode()),
            "entity_count": len(revisions),
            "audit_route": "https://www.wikidata.org/wiki/Special:EntityData/{QID}.json?revision={lastrevid}",
        },
        "archive_path": repo_relative(archive),
        "archive_sha256": sha256_file(archive),
        "archive_bytes": archive.stat().st_size,
        "license_files": [],
        "retrieval": "wbgetentities batches recorded in requests.json (props=info|labels|aliases|descriptions|sitelinks, "
        "languages=en, sitefilter=enwiki); per-entity lastrevid pins each revision",
    }


LICENSES = {
    "oewn-2025": {
        "spdx": "CC-BY-4.0",
        "notice": "Open English WordNet (2025 edition), (c) The Open English WordNet Team, CC BY 4.0; "
        "derived from Princeton WordNet under the WordNet License (WNDB_License.txt).",
        "evidence": "LICENSE.md and WNDB_License.txt at tag 2025-edition",
        "imported_fields": "synset members, sense keys, part of speech, lexicographer file, first definition, "
        "domain_topic, similar, sense antonym",
    },
    "github-explore-c8d7bc5": {
        "spdx": "CC-BY-4.0",
        "notice": "Content based on github.com/github/explore used under the CC-BY-4.0 license.",
        "evidence": "LICENSE.txt and notices.md at the pinned commit",
        "imported_fields": "topic slug, display_name, aliases, related, wikipedia_url (for entity selection only)",
    },
    "wikidata-sel1": {
        "spdx": "CC0-1.0",
        "notice": "Wikidata structured data (labels, aliases, descriptions) is available under CC0 1.0.",
        "evidence": "https://www.wikidata.org/wiki/Wikidata:Licensing (structured data namespace is CC0); "
        "only labels/aliases/descriptions are imported, never Wikipedia text",
        "imported_fields": "English label, English aliases, English description, entity id and lastrevid",
    },
    "abbrcode-bd33b1c": {
        "spdx": "MIT",
        "notice": "abbreviations-in-code, Copyright (c) 2022 Gabor Kiss-Vamosi, MIT License.",
        "evidence": "LICENCE at the pinned commit",
        "imported_fields": "word, abbr, degree, context from data/abbrs/.json",
    },
    "se-abbr-8e1d77e": {
        "spdx": "CC0-1.0",
        "notice": "software-engineering-abbreviations-acronyms by wuyichen24, CC0 1.0 Universal.",
        "evidence": "LICENSE at the pinned commit",
        "imported_fields": "abbreviation and full name from each dictionary entry title, first overview line",
    },
}


def pack() -> None:
    entries = [pack_wordnet()]
    entries += [pack_git_source(key) for key in GIT_SOURCES]
    entries.append(pack_wikidata())
    for entry in entries:
        entry["license"] = LICENSES[entry["source_id"]]
    write_json(
        SOURCES_LOCK_PATH,
        {
            "lock_version": 1,
            "note": "Archive paths are under the ignored .codemap/ directory; hashes pin their exact bytes.",
            "excluded_sources": [
                {"name": "ConceptNet", "reason": "CC BY-SA"},
                {"name": "Stack Overflow tag synonyms", "reason": "CC BY-SA"},
                {"name": "SCANL abbreviation artifact", "reason": "no license"},
                {"name": "SEthesaurus data", "reason": "license unverified"},
                {"name": "SWordNet data", "reason": "license unverified; Linux-kernel subset"},
                {"name": "Moby Thesaurus II", "reason": "general-English senses; excluded by the parent brief"},
                {"name": "Wikipedia article text", "reason": "CC BY-SA; only Wikidata structured fields are imported"},
                {"name": "public-code co-occurrence corpus", "reason": "not mined in phase 1; corpus support stays absent"},
            ],
            "sources": entries,
        },
    )
    print(f"wrote {repo_relative(SOURCES_LOCK_PATH)} with {len(entries)} sources")


def verify() -> int:
    from rtpoc_common import REPO_ROOT

    lock = read_json(SOURCES_LOCK_PATH)
    failures = 0
    for entry in lock["sources"]:
        for path_key, hash_key in (("archive_path", "archive_sha256"), ("license_archive_path", "license_archive_sha256")):
            if path_key not in entry:
                continue
            path = REPO_ROOT / entry[path_key]
            actual = sha256_file(path) if path.exists() else None
            status = "ok" if actual == entry[hash_key] else "MISMATCH"
            failures += status != "ok"
            print(f"{status} {entry['source_id']} {entry[path_key]}")
    return 1 if failures else 0


def archived_wikidata_entities() -> dict[str, dict]:
    target = RAW_DIR / "wikidata"
    entities: dict[str, dict] = {}
    for request in read_json(target / "requests.json")["requests"]:
        for entity_id, entity in json.loads((target / request["file"]).read_bytes()).get("entities", {}).items():
            if "missing" not in entity:
                entities[entity_id] = entity
    return entities


def plan_revision_audit(sample: int, seed: int) -> None:
    """List Special:EntityData URLs that re-read the diagnostic and a seeded sample at their pinned revisions."""
    target = RAW_DIR / "wikidata"
    entities = archived_wikidata_entities()
    chosen = set(WIKIDATA_DIAGNOSTIC_IDS)
    rng = random.Random(seed)
    chosen.update(rng.sample(sorted(set(entities) - chosen), min(sample, len(entities) - len(chosen))))
    (target / "audit").mkdir(parents=True, exist_ok=True)
    lines = [
        f"audit/{entity_id}.json https://www.wikidata.org/wiki/Special:EntityData/{entity_id}.json"
        f"?revision={entities[entity_id]['lastrevid']}\n"
        for entity_id in sorted(chosen)
    ]
    (target / "audit-urls.txt").write_text("".join(lines), encoding="utf-8")
    write_json(target / "revision-audit-plan.json", {"seed": seed, "sample": sample, "entities": sorted(chosen)})


def compare_revision_audit() -> int:
    target = RAW_DIR / "wikidata"
    entities = archived_wikidata_entities()
    plan = read_json(target / "revision-audit-plan.json")

    def english(entity: dict) -> tuple:
        return (
            entity.get("labels", {}).get("en", {}).get("value"),
            tuple(alias["value"] for alias in entity.get("aliases", {}).get("en", [])),
            entity.get("descriptions", {}).get("en", {}).get("value"),
        )

    results = []
    for entity_id in plan["entities"]:
        archived = entities[entity_id]
        fetched = json.loads((target / "audit" / f"{entity_id}.json").read_bytes())["entities"][entity_id]
        same = fetched.get("lastrevid") == archived["lastrevid"] and english(fetched) == english(archived)
        results.append({"entity": entity_id, "revision": archived["lastrevid"], "matches": same})
        print(f"{'ok' if same else 'MISMATCH'} {entity_id} r{archived['lastrevid']}")
    write_json(target / "revision-audit.json", {**plan, "results": results})
    return 0 if all(result["matches"] for result in results) else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in (
        "plan-wikidata",
        "plan-wikidata-redirects",
        "plan-wikidata-resolved",
        "record-wikidata",
        "pack",
        "verify",
        "compare-revision-audit",
    ):
        sub.add_parser(name)
    audit = sub.add_parser("plan-revision-audit")
    audit.add_argument("--sample", type=int, default=20)
    audit.add_argument("--seed", type=int, default=20261006)
    args = parser.parse_args()
    if args.command == "plan-wikidata":
        plan_wikidata()
    elif args.command == "plan-wikidata-redirects":
        plan_wikidata_redirects()
    elif args.command == "plan-wikidata-resolved":
        plan_wikidata_resolved()
    elif args.command == "record-wikidata":
        record_wikidata()
    elif args.command == "pack":
        pack()
    elif args.command == "verify":
        return verify()
    elif args.command == "plan-revision-audit":
        plan_revision_audit(args.sample, args.seed)
    else:
        return compare_revision_audit()
    return 0


if __name__ == "__main__":
    sys.exit(main())
