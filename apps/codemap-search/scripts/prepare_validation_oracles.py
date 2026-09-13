#!/usr/bin/env python3
"""Prepare isolated, pinned parser helpers for public-repository validation."""

import argparse
import hashlib
from pathlib import Path
import shutil
import subprocess
from urllib.request import urlopen
import xml.etree.ElementTree as ET

from public_validation import DATA, save_json, sha256

MAVEN = "https://repo.maven.apache.org/maven2"
NS = {"m": "http://maven.apache.org/POM/4.0.0"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", required=True, type=Path)
    args = parser.parse_args()
    root = args.cache.expanduser().resolve() / "oracles"
    jars = root / "jars"
    classes = root / "classes"
    jars.mkdir(parents=True, exist_ok=True)
    classes.mkdir(exist_ok=True)
    artifacts, visited = [], set()

    def artifact(group, name, version, transitive=True):
        if group == "org.scala-lang":
            version = "2.13.15"
        key = (group, name, version)
        if key in visited:
            return
        visited.add(key)
        base = f"{MAVEN}/{group.replace('.', '/')}/{name}/{version}/{name}-{version}"
        path = jars / f"{name}-{version}.jar"
        checksum = urlopen(base + ".jar.sha1", timeout=60).read().decode().strip().split()[0]
        if not path.exists() or hashlib.sha1(path.read_bytes()).hexdigest() != checksum:
            path.write_bytes(urlopen(base + ".jar", timeout=60).read())
        if hashlib.sha1(path.read_bytes()).hexdigest() != checksum:
            raise RuntimeError(f"Artifact checksum mismatch: {path}")
        artifacts.append({"coordinates": ":".join(key), "url": base + ".jar", "sha256": sha256(path), "path": str(path)})
        if not transitive:
            return
        pom = ET.fromstring(urlopen(base + ".pom", timeout=60).read())
        for dependency in pom.findall("m:dependencies/m:dependency", NS):
            def value(field):
                return dependency.findtext("m:" + field, namespaces=NS)
            if value("scope") in {"test", "provided"} or value("optional") == "true":
                continue
            if not value("version") or "${" in value("version"):
                raise RuntimeError(f"Unresolved dependency in {key}: {ET.tostring(dependency)}")
            artifact(value("groupId"), value("artifactId"), value("version"))

    artifact("org.scalameta", "scalameta_2.13", "4.17.3")
    artifact("org.scala-lang", "scala-compiler", "2.13.15", False)
    artifact("org.scala-lang", "scala-reflect", "2.13.15", False)
    artifact("org.codehaus.groovy", "groovy", "3.0.25", False)
    classpath = ":".join(item["path"] for item in artifacts)
    subprocess.run(["javac", "-cp", classpath, "-d", str(classes), str(DATA / "GroovyOracle.java")], check=True)
    subprocess.run(["java", "-Dscala.usejavacp=true", "-cp", classpath, "scala.tools.nsc.Main", "-d", str(classes), str(DATA / "ScalaOracle.scala")], check=True)
    dart = root / "dart"
    dart.mkdir(exist_ok=True)
    for name in ("pubspec.yaml", "main.dart"):
        shutil.copy(DATA / "dart_oracle" / name, dart / name)
    if (DATA / "dart_oracle/pubspec.lock").exists():
        shutil.copy(DATA / "dart_oracle/pubspec.lock", dart / "pubspec.lock")
    subprocess.run(["dart", "pub", "get", "--enforce-lockfile"], cwd=dart, check=True)
    subprocess.run(["dart", "compile", "exe", "main.dart", "-o", str(root / "dart-oracle")], cwd=dart, check=True)
    save_json(root / "manifest.json", {"artifacts": artifacts, "classpath": str(classes) + ":" + classpath,
        "groovy_source_sha256": sha256(DATA / "GroovyOracle.java"), "scala_source_sha256": sha256(DATA / "ScalaOracle.scala"),
        "dart_source_sha256": sha256(DATA / "dart_oracle/main.dart"), "dart_lock_sha256": sha256(dart / "pubspec.lock"),
        "dart_binary_sha256": sha256(root / "dart-oracle")})
    print(root / "manifest.json")


if __name__ == "__main__":
    main()
