#!/usr/bin/env python3
"""Build and verify a reproducible NORTEX source archive (Python 3.11+)."""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import tomllib
import zipfile
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CONFIG = ROOT / "packaging.toml"
CHUNK_SIZE = 1024 * 1024


class PackageError(RuntimeError):
    """Packaging or validation failure."""


class BuildError(PackageError):
    """A configured verification command failed."""


@dataclass
class BuildResult:
    name: str
    command: list[str]
    status: str
    return_code: int | None
    duration_seconds: float
    output: str


def log(message: str) -> None:
    print(message, flush=True)


def read_config(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as stream:
            config = tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise PackageError(f"Cannot read packaging config {path}: {exc}") from exc
    for section in ("package", "paths", "manifest", "rules", "security",
                    "reproducibility", "verification", "release", "report"):
        if not isinstance(config.get(section), dict):
            raise PackageError(f"Missing or invalid [{section}] section in {path}")
    return config


def run_command(
    name: str, command: list[str], cwd: Path, timeout: int
) -> BuildResult:
    log(f"Verification: {name}")
    started = time.monotonic()
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
            check=False,
        )
        duration = time.monotonic() - started
        status = "PASSED" if result.returncode == 0 else "FAILED"
        build_result = BuildResult(
            name=name,
            command=command,
            status=status,
            return_code=result.returncode,
            duration_seconds=round(duration, 3),
            output=(result.stdout + "\n" + result.stderr)[-12000:],
        )
    except subprocess.TimeoutExpired as exc:
        build_result = BuildResult(
            name=name,
            command=command,
            status="TIMEOUT",
            return_code=None,
            duration_seconds=round(time.monotonic() - started, 3),
            output=str(exc),
        )
    except OSError as exc:
        build_result = BuildResult(
            name=name,
            command=command,
            status="ERROR",
            return_code=None,
            duration_seconds=round(time.monotonic() - started, 3),
            output=str(exc),
        )
    if build_result.status != "PASSED":
        raise BuildError(
            f"{name} failed: {build_result.status}\n{build_result.output}"
        )
    return build_result


def verification_commands(config: dict[str, Any]) -> list[tuple[str, list[str]]]:
    verification = config["verification"]
    all_targets = bool(verification.get("cargo_all_targets", False))
    all_features = bool(verification.get("cargo_all_features", False))
    deny_warnings = bool(verification.get("cargo_deny_warnings", False))
    commands: list[tuple[str, list[str]]] = []

    if verification.get("run_cargo_fmt", False):
        commands.append(("CARGO_FMT", ["cargo", "fmt", "--all", "--", "--check"]))

    if verification.get("run_cargo_check", False):
        command = ["cargo", "check", "--workspace", "--locked"]
        if all_targets:
            command.append("--all-targets")
        if all_features:
            command.append("--all-features")
        commands.append(("CARGO_CHECK", command))

    if verification.get("run_cargo_clippy", False):
        command = ["cargo", "clippy", "--workspace", "--locked"]
        if all_targets:
            command.append("--all-targets")
        if all_features:
            command.append("--all-features")
        if deny_warnings:
            command.extend(["--", "-D", "warnings"])
        commands.append(("CARGO_CLIPPY", command))

    if verification.get("run_cargo_test", False):
        command = ["cargo", "test", "--workspace", "--locked"]
        if all_targets:
            command.append("--all-targets")
        if all_features:
            command.append("--all-features")
        commands.append(("CARGO_TEST", command))

    return commands


def check_git_requirements(config: dict[str, Any], allow_non_git: bool) -> dict[str, Any]:
    release = config["release"]
    result: dict[str, Any] = {"available": False, "commit": None, "status": None}
    try:
        top = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        top = None

    if top is None or top.returncode != 0:
        if release.get("require_git_repository", False) and not allow_non_git:
            raise PackageError(
                "A Git repository is required. Use --allow-non-git only for a "
                "non-release development archive."
            )
        return result

    if Path(top.stdout.strip()).resolve() != ROOT.resolve():
        raise PackageError("The packaging root is not the Git repository root.")
    result["available"] = True
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True,
        text=True, check=False,
    )
    result["commit"] = commit.stdout.strip() if commit.returncode == 0 else None
    status = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=ROOT, capture_output=True, text=True, check=False,
    )
    if status.returncode != 0:
        raise PackageError(f"Cannot inspect Git worktree: {status.stderr.strip()}")
    result["status"] = status.stdout.splitlines()
    if release.get("require_clean_worktree", False) and result["status"]:
        raise PackageError("Release requires a clean Git worktree.")
    if not release.get("allow_untracked_files", True):
        untracked = [line for line in result["status"] if line.startswith("??")]
        if untracked:
            raise PackageError("Untracked files are not allowed in this package.")
    return result


def normalized_relpath(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def is_excluded(relpath: str, config: dict[str, Any]) -> bool:
    rules = config["rules"]
    insensitive = bool(rules.get("case_insensitive_matching", True))
    candidate = relpath.lower() if insensitive else relpath
    basename = PurePosixPath(candidate).name

    excluded_dirs = {
        str(value).lower() if insensitive else str(value)
        for value in rules.get("exclude_dirs", [])
    }
    if any(part in excluded_dirs for part in PurePosixPath(candidate).parts[:-1]):
        return True

    excluded_names = [
        str(value).lower() if insensitive else str(value)
        for value in rules.get("exclude_filenames", [])
    ]
    if basename in excluded_names:
        return True

    suffixes = [
        str(value).lower() if insensitive else str(value)
        for value in rules.get("exclude_extensions", [])
    ]
    suffix = Path(basename).suffix
    if (suffix.lower() if insensitive else suffix) in suffixes:
        return True

    for value in rules.get("exclude_paths", []):
        pattern = str(value).replace("\\", "/")
        pattern = pattern.lower() if insensitive else pattern
        if fnmatch.fnmatchcase(candidate, pattern):
            return True
    return False


def blocked_path(relpath: str, config: dict[str, Any]) -> bool:
    security = config["security"]
    if not security.get("scan_filenames", True):
        return False
    basename = PurePosixPath(relpath).name
    allowed = {str(item).lower() for item in security.get("allowed_security_patterns", [])}
    if basename.lower() in allowed:
        return False
    return any(
        fnmatch.fnmatchcase(relpath.lower(), str(pattern).lower())
        or fnmatch.fnmatchcase(basename.lower(), str(pattern).lower())
        for pattern in security.get("blocked_patterns", [])
    )


SECRET_PATTERNS = (
    re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----[\s\S]{40,}?-----END (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    re.compile(r"\bgh[pousr]_[A-Za-z0-9]{30,}\b"),
    re.compile(r"\bsk-[A-Za-z0-9]{32,}\b"),
)


def file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(CHUNK_SIZE):
            digest.update(chunk)
    return digest.hexdigest()


def inspect_file(path: Path, relpath: str, config: dict[str, Any]) -> None:
    manifest = config["manifest"]
    security = config["security"]
    max_bytes = int(manifest.get("max_file_size_mb", security.get("max_file_size_mb", 100))) * 1024 * 1024
    if path.stat().st_size > max_bytes:
        raise PackageError(f"File exceeds configured size limit: {relpath}")
    if blocked_path(relpath, config):
        raise PackageError(f"Blocked filename in package input: {relpath}")
    if not security.get("enabled", True) or not security.get("scan_file_contents", True):
        return
    try:
        with path.open("rb") as stream:
            sample = stream.read(8192)
            is_binary = b"\0" in sample
            if manifest.get("reject_unexpected_binaries", False) and is_binary:
                allowed = [
                    str(pattern).lower()
                    for pattern in manifest.get("allowed_binary_paths", [])
                ]
                if not any(fnmatch.fnmatchcase(relpath.lower(), pattern) for pattern in allowed):
                    raise PackageError(f"Unexpected binary file: {relpath}")
            if is_binary:
                return
            stream.seek(0)
            content = stream.read()
        text = content.decode("utf-8", errors="ignore")
    except OSError as exc:
        raise PackageError(f"Cannot inspect {relpath}: {exc}") from exc
    if any(pattern.search(text) for pattern in SECRET_PATTERNS):
        raise PackageError(f"Potential secret detected in: {relpath}")


def collect_files(config: dict[str, Any]) -> list[Path]:
    files: list[Path] = []
    for current, dirs, names in os.walk(ROOT, topdown=True, followlinks=False):
        current_path = Path(current)
        kept_dirs: list[str] = []
        for name in sorted(dirs):
            path = current_path / name
            relpath = normalized_relpath(path)
            if path.is_symlink():
                if config["manifest"].get("reject_symlinks", True):
                    raise PackageError(f"Symlink directory is not allowed: {relpath}")
                continue
            if not is_excluded(relpath + "/placeholder", config):
                kept_dirs.append(name)
        dirs[:] = kept_dirs

        for name in sorted(names):
            path = current_path / name
            relpath = normalized_relpath(path)
            if is_excluded(relpath, config):
                continue
            if path.is_symlink():
                if config["manifest"].get("reject_symlinks", True):
                    raise PackageError(f"Symlink is not allowed: {relpath}")
                continue
            if not path.is_file():
                continue
            inspect_file(path, relpath, config)
            files.append(path)

    files.sort(key=lambda item: normalized_relpath(item))
    required = {str(value).replace("\\", "/") for value in config["manifest"].get("required_files", [])}
    present = {normalized_relpath(path) for path in files}
    missing = sorted(required - present)
    if missing:
        raise PackageError("Required files are missing or excluded: " + ", ".join(missing))
    for directory in config["manifest"].get("required_directories", []):
        if not (ROOT / directory).is_dir():
            raise PackageError(f"Required directory does not exist: {directory}")
    return files


def validate_zip_member(name: str) -> None:
    if "\\" in name:
        raise PackageError(f"Unsafe archive path: {name}")
    path = PurePosixPath(name)
    if path.is_absolute() or any(part in ("", ".", "..") for part in path.parts):
        raise PackageError(f"Unsafe archive path: {name}")


def fixed_zip_time(timestamp: int) -> tuple[int, int, int, int, int, int]:
    moment = datetime.fromtimestamp(timestamp, timezone.utc)
    year = min(2107, max(1980, moment.year))
    return year, moment.month, moment.day, moment.hour, moment.minute, moment.second // 2 * 2


def set_zip_compression_level(info: zipfile.ZipInfo, level: int) -> None:
    if hasattr(info, "compress_level"):
        info.compress_level = level
    else:
        info._compresslevel = level


def write_manifest(files: list[Path], config: dict[str, Any]) -> bytes:
    entries = []
    for path in files:
        entry: dict[str, Any] = {
            "path": normalized_relpath(path),
            "size": path.stat().st_size,
        }
        if config["report"].get("include_file_hashes", True):
            entry["sha256"] = file_hash(path)
        entries.append(entry)
    return (json.dumps({"files": entries}, indent=2, sort_keys=True) + "\n").encode("utf-8")


def create_archive(files: list[Path], archive_path: Path, config: dict[str, Any]) -> list[str]:
    reproducibility = config["reproducibility"]
    manifest_name = str(config["paths"].get("manifest_filename", "MANIFEST.json"))
    archive_names = [normalized_relpath(path) for path in files]
    if manifest_name in archive_names:
        raise PackageError(f"Manifest path collides with project file: {manifest_name}")
    if len(set(archive_names)) != len(archive_names):
        raise PackageError("Duplicate archive paths detected.")
    for name in archive_names:
        validate_zip_member(name)
    archive_names.append(manifest_name)

    compression_name = str(reproducibility.get("compression", "deflate")).lower()
    if compression_name != "deflate":
        raise PackageError(f"Unsupported compression method: {compression_name}")
    level = int(reproducibility.get("compression_level", 9))
    if not 0 <= level <= 9:
        raise PackageError("Deflate compression level must be between 0 and 9.")
    timestamp = int(reproducibility.get("fixed_timestamp", 1767225600))
    zip_time = fixed_zip_time(timestamp)
    manifest_data = write_manifest(files, config)

    with zipfile.ZipFile(
        archive_path, "w", compression=zipfile.ZIP_DEFLATED,
        compresslevel=level, strict_timestamps=False,
    ) as archive:
        archive.comment = str(reproducibility.get("archive_comment", "")).encode("utf-8")
        for path in files:
            name = normalized_relpath(path)
            info = zipfile.ZipInfo(name, date_time=zip_time)
            info.compress_type = zipfile.ZIP_DEFLATED
            set_zip_compression_level(info, level)
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            with path.open("rb") as source, archive.open(info, "w") as destination:
                shutil.copyfileobj(source, destination, length=CHUNK_SIZE)
        info = zipfile.ZipInfo(manifest_name, date_time=zip_time)
        info.compress_type = zipfile.ZIP_DEFLATED
        set_zip_compression_level(info, level)
        info.external_attr = (stat.S_IFREG | 0o644) << 16
        archive.writestr(info, manifest_data)
    return archive_names


def verify_archive(archive_path: Path, expected_names: list[str]) -> None:
    try:
        with zipfile.ZipFile(archive_path, "r") as archive:
            bad_member = archive.testzip()
            if bad_member is not None:
                raise PackageError(f"ZIP CRC check failed for {bad_member}")
            actual = archive.namelist()
            if len(actual) != len(set(actual)):
                raise PackageError("Archive contains duplicate member paths.")
            if set(actual) != set(expected_names):
                missing = sorted(set(expected_names) - set(actual))
                unexpected = sorted(set(actual) - set(expected_names))
                raise PackageError(
                    f"Archive file list mismatch; missing={missing}, unexpected={unexpected}"
                )
            for name in actual:
                validate_zip_member(name)
                info = archive.getinfo(name)
                if stat.S_ISLNK(info.external_attr >> 16):
                    raise PackageError(f"Symlink entry found in archive: {name}")
    except zipfile.BadZipFile as exc:
        raise PackageError(f"Invalid ZIP archive: {exc}") from exc


def clean_unpack_build(archive_path: Path, config: dict[str, Any], timeout: int) -> BuildResult:
    command = config["verification"].get(
        "clean_unpack_build_command",
        ["cargo", "build", "--workspace", "--locked"],
    )
    if not isinstance(command, list) or not command or not all(isinstance(part, str) for part in command):
        raise PackageError("clean_unpack_build_command must be a non-empty string array.")
    with tempfile.TemporaryDirectory(prefix="nortex-clean-build-") as temp_name:
        destination = Path(temp_name)
        with zipfile.ZipFile(archive_path, "r") as archive:
            for info in archive.infolist():
                validate_zip_member(info.filename)
                mode = info.external_attr >> 16
                if stat.S_ISLNK(mode):
                    raise PackageError(f"Refusing to extract symlink: {info.filename}")
                target = (destination / Path(*PurePosixPath(info.filename).parts)).resolve()
                if not target.is_relative_to(destination.resolve()):
                    raise PackageError(f"Archive path escapes extraction directory: {info.filename}")
            archive.extractall(destination)
        return run_command("CLEAN_UNPACK_BUILD", command, destination, timeout)


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = path.with_name(path.name + ".tmp")
    temp_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temp_path, path)


def package(config_path: Path, allow_non_git: bool, skip_verification: bool) -> Path:
    config = read_config(config_path)
    if allow_non_git and config["package"].get("source_type") != "dev_archive":
        raise PackageError(
            "--allow-non-git is restricted to package.source_type = 'dev_archive'."
        )
    git_info = check_git_requirements(config, allow_non_git)
    files = collect_files(config)
    paths = config["paths"]
    package_info = config["package"]
    output_dir = ROOT / str(paths.get("output_dir", "dist"))
    output_dir.mkdir(parents=True, exist_ok=True)
    archive_name = str(paths.get("archive_pattern", "{name}-{version}-{suffix}.zip")).format(
        name=package_info["name"],
        version=package_info["version"],
        suffix=package_info.get("archive_suffix", "src"),
    )
    archive_path = output_dir / archive_name
    if archive_path.exists() and not config["release"].get("overwrite_existing_archive", False):
        raise PackageError(f"Archive already exists: {archive_path}")
    report_name = str(paths.get("report_pattern", "{name}-{version}-build.json")).format(
        name=package_info["name"], version=package_info["version"]
    )
    report_path = output_dir / report_name
    timeout = int(config["verification"].get("command_timeout_seconds", 600))
    results: list[BuildResult] = []
    verification_enabled = config["verification"].get("enabled", True)

    if skip_verification:
        if config["release"].get("require_all_gates_passed", True):
            raise PackageError(
                "--skip-verification is forbidden while require_all_gates_passed is true."
            )
    elif verification_enabled:
        for name, command in verification_commands(config):
            results.append(run_command(name, command, ROOT, timeout))

    output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        prefix=".nortex-package-", suffix=".zip", dir=output_dir, delete=False
    ) as stream:
        temporary_archive = Path(stream.name)
    try:
        expected_names = create_archive(files, temporary_archive, config)
        verify_archive(temporary_archive, expected_names)
        if config["verification"].get("run_clean_unpack_build", False):
            if skip_verification:
                raise PackageError("Clean unpack build cannot be skipped for this configuration.")
            results.append(clean_unpack_build(temporary_archive, config, timeout))
        os.replace(temporary_archive, archive_path)
    finally:
        temporary_archive.unlink(missing_ok=True)

    checksum_path = archive_path.with_suffix(archive_path.suffix + str(paths.get("checksum_suffix", ".sha256")))
    checksum = file_hash(archive_path)
    checksum_path.write_text(f"{checksum}  {archive_path.name}\n", encoding="ascii")

    report = {
        "name": package_info["name"],
        "version": package_info["version"],
        "archive": archive_path.name,
        "archive_bytes": archive_path.stat().st_size,
        "archive_sha256": checksum,
        "source_file_count": len(files),
        "source_bytes": sum(path.stat().st_size for path in files),
        "manifest_path": paths.get("manifest_filename", "MANIFEST.json"),
        "git": git_info,
        "verification": [asdict(item) for item in results],
        "verification_skipped": skip_verification,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
    }
    if config["report"].get("enabled", True):
        write_json_atomic(report_path, report)
    log(f"Archive: {archive_path}")
    log(f"Files: {len(files)}; size: {archive_path.stat().st_size} bytes")
    log(f"SHA-256: {checksum}")
    return archive_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument(
        "--allow-non-git",
        action="store_true",
        help="Permit a non-Git development archive; does not waive other release gates.",
    )
    parser.add_argument(
        "--skip-verification",
        action="store_true",
        help="Skip configured Cargo checks (only allowed when release gates permit it).",
    )
    args = parser.parse_args()
    try:
        package(args.config.resolve(), args.allow_non_git, args.skip_verification)
    except PackageError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
