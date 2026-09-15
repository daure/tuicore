"""Retrieve the published crate and verify its registry checksum."""

import hashlib
from pathlib import Path
import tomllib
import urllib.request

from release import registry_version


def fetch_crate(version, destination):
    metadata = registry_version("tuicore", version)
    if metadata is None or metadata["yanked"]:
        raise ValueError(f"Tuicore {version} must be published and non-yanked")
    request = urllib.request.Request(
        f"https://static.crates.io/crates/tuicore/tuicore-{version}.crate",
        headers={"User-Agent": "tuicore-release"},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read()
    checksum = hashlib.sha256(data).hexdigest()
    if checksum != metadata["checksum"]:
        raise ValueError("Published crate checksum does not match crates.io metadata")
    destination.mkdir(parents=True, exist_ok=True)
    artifact = destination / f"tuicore-{version}.crate"
    artifact.write_bytes(data)
    artifact.with_suffix(".crate.sha256").write_text(f"{checksum}  {artifact.name}\n")
    print(f"Verified published package: {artifact.name}")


if __name__ == "__main__":
    version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
    fetch_crate(version, Path("target/distrib"))
