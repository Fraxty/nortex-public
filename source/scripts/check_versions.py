import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def fail(message: str) -> None:
    print(f"Version check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


workspace_manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
workspace = workspace_manifest.get("workspace", {})
version = workspace.get("package", {}).get("version")
if not isinstance(version, str):
    fail("workspace.package.version is missing or is not a string")

members = []
for pattern in workspace.get("members", []):
    for member_dir in sorted(ROOT.glob(pattern)):
        manifest_path = member_dir / "Cargo.toml"
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        package = manifest.get("package", {})
        name = package.get("name")
        declared_version = package.get("version")
        if isinstance(declared_version, dict) and declared_version.get("workspace"):
            declared_version = version
        if not isinstance(name, str):
            fail(f"{manifest_path.relative_to(ROOT)} has no package.name")
        if declared_version != version:
            fail(
                f"{name} declares {declared_version!r}; "
                f"workspace version is {version!r}"
            )
        members.append(name)

if not members:
    fail("workspace has no package members")

lockfile = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
locked_packages = lockfile.get("package", [])
for name in members:
    locked_versions = [
        package.get("version")
        for package in locked_packages
        if package.get("name") == name
    ]
    if locked_versions != [version]:
        fail(f"{name} lockfile versions are {locked_versions!r}, expected [{version!r}]")

print(
    f"Workspace version {version} is consistent across "
    f"{len(members)} package manifests and Cargo.lock."
)
