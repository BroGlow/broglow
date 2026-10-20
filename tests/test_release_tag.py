"""Release tags identify committed Cargo versions without changing dependencies."""

import importlib.util
import json
from pathlib import Path
import subprocess
import tomllib

import pytest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "release_tag", ROOT / "scripts/release-tag.py"
)
release_tag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release_tag)
package_spec = importlib.util.spec_from_file_location(
    "release_tag_package", ROOT / "scripts/package-macos.py"
)
package = importlib.util.module_from_spec(package_spec)
package_spec.loader.exec_module(package)


def git(repository: Path, *args: str) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=repository, text=True, stderr=subprocess.PIPE
    ).strip()


@pytest.fixture
def repository(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setattr(release_tag, "ROOT", tmp_path)
    monkeypatch.setattr(package, "ROOT", tmp_path)
    git(tmp_path, "init")
    for key, value in (
        ("user.name", "Release Check"),
        ("user.email", "release-check@example.invalid"),
        ("commit.gpgsign", "false"),
        ("tag.gpgsign", "false"),
        ("core.hooksPath", ".git/hooks"),
    ):
        git(tmp_path, "config", key, value)
    (tmp_path / "src").mkdir()
    (tmp_path / "src/main.rs").write_text("fn main() {}\n")
    (tmp_path / "Cargo.toml").write_text(
        '[package]\nname = "broglow"\nversion = "1.0.1" # app version\n'
        'edition = "2021"\n\n[dependencies]\n'
        'support = { path = "support", version = "1.0.1" }\n'
    )
    (tmp_path / "support/src").mkdir(parents=True)
    (tmp_path / "support/src/lib.rs").write_text("// Local dependency.\n")
    (tmp_path / "support/Cargo.toml").write_text(
        '[package]\nname = "support"\nversion = "1.0.1"\nedition = "2021"\n'
    )
    subprocess.run(
        ["cargo", "generate-lockfile", "--offline"],
        cwd=tmp_path,
        check=True,
        capture_output=True,
    )
    git(tmp_path, "add", ".")
    git(tmp_path, "commit", "-m", "Initial source")
    return tmp_path


def source_state(repository: Path) -> tuple[str, bytes, bytes]:
    return (
        git(repository, "rev-parse", "HEAD"),
        (repository / "Cargo.toml").read_bytes(),
        (repository / "Cargo.lock").read_bytes(),
    )


def test_tag_commits_only_matching_root_versions(repository: Path) -> None:
    initial, manifest, lock = source_state(repository)
    release_tag.release("v1.0.2")

    assert (repository / "Cargo.toml").read_bytes() == manifest.replace(
        b'version = "1.0.1" # app version', b'version = "1.0.2" # app version'
    )
    assert (repository / "Cargo.lock").read_bytes() == lock.replace(
        b'name = "broglow"\nversion = "1.0.1"',
        b'name = "broglow"\nversion = "1.0.2"',
    )
    assert git(repository, "diff", "--name-only", initial, "HEAD").splitlines() == [
        "Cargo.lock",
        "Cargo.toml",
    ]
    assert git(repository, "cat-file", "-t", "v1.0.2") == "tag"
    assert git(repository, "status", "--porcelain") == ""
    assert git(repository, "remote") == ""
    version = tomllib.loads((repository / "Cargo.toml").read_text())["package"][
        "version"
    ]
    assert package.release_identity(version, "v1.0.2") == (
        git(repository, "rev-parse", "HEAD"),
        False,
    )
    metadata = json.loads(
        subprocess.check_output(
            [
                "cargo",
                "metadata",
                "--offline",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            ],
            cwd=repository,
        )
    )
    assert metadata["packages"][0]["version"] == "1.0.2"


def test_matching_version_creates_tag_without_empty_commit(repository: Path) -> None:
    before = source_state(repository)
    release_tag.release("v1.0.1")
    assert source_state(repository) == before
    assert git(repository, "rev-parse", "v1.0.1^{commit}") == before[0]
    assert git(repository, "cat-file", "-t", "v1.0.1") == "tag"


@pytest.mark.parametrize(
    "tag",
    [
        "1.0.2",
        "v0.9.9",
        "v01.0.2",
        "v1.00.2",
        "v1.0.02",
        "v1.0.2-rc.1",
        "v1.0.2+build",
        "v1.0.2\n",
    ],
)
def test_invalid_tag_refused_without_changes(repository: Path, tag: str) -> None:
    before = source_state(repository)
    with pytest.raises(RuntimeError, match="stable vX.Y.Z"):
        release_tag.release(tag)
    assert source_state(repository) == before
    assert git(repository, "tag", "--list") == ""


def test_lower_version_refused(repository: Path) -> None:
    before = source_state(repository)
    with pytest.raises(RuntimeError, match="Refusing to lower"):
        release_tag.release("v1.0.0")
    assert source_state(repository) == before
    assert git(repository, "tag", "--list") == ""


def test_existing_tag_never_moved(repository: Path) -> None:
    before = source_state(repository)
    git(repository, "tag", "v1.0.2")
    with pytest.raises(RuntimeError, match="already exists"):
        release_tag.release("v1.0.2")
    assert source_state(repository) == before
    assert git(repository, "rev-parse", "v1.0.2") == before[0]


@pytest.mark.parametrize("dirty", ["tracked", "staged", "untracked"])
def test_dirty_checkout_refused(repository: Path, dirty: str) -> None:
    before = source_state(repository)
    path = repository / ("new-file" if dirty == "untracked" else "src/main.rs")
    path.write_text("Uncommitted source\n")
    if dirty == "staged":
        git(repository, "add", "src/main.rs")
    status = git(repository, "status", "--porcelain")
    with pytest.raises(RuntimeError, match="all source changes"):
        release_tag.release("v1.0.2")
    assert source_state(repository) == before
    assert git(repository, "status", "--porcelain") == status
    assert git(repository, "tag", "--list") == ""


def test_mismatched_lock_refused_even_if_cargo_metadata_accepts_it(
    repository: Path,
) -> None:
    path = repository / "Cargo.lock"
    path.write_text(
        path.read_text().replace(
            'name = "broglow"\nversion = "1.0.1"',
            'name = "broglow"\nversion = "1.0.0"',
        )
    )
    git(repository, "commit", "-am", "Stale lock")
    before = source_state(repository)
    with pytest.raises(RuntimeError, match="must already match"):
        release_tag.release("v1.0.2")
    assert source_state(repository) == before
    assert git(repository, "tag", "--list") == ""


def test_cargo_failure_restores_versions_before_any_commit_or_tag(
    repository: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    before = source_state(repository)
    real_run = subprocess.run

    def fail_cargo(args: list[str], **kwargs):
        if args[0] == "cargo":
            assert kwargs["stdout"] == subprocess.DEVNULL
            assert kwargs["stderr"] == subprocess.DEVNULL
            raise subprocess.CalledProcessError(101, args)
        return real_run(args, **kwargs)

    monkeypatch.setattr(release_tag.subprocess, "run", fail_cargo)
    with pytest.raises(
        RuntimeError, match="original versions restored.*No commit or tag"
    ):
        release_tag.release("v1.0.2")
    assert source_state(repository) == before
    assert git(repository, "status", "--porcelain") == ""
    assert git(repository, "tag", "--list") == ""


def test_failed_tag_reports_retained_version_commit(
    repository: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    before = source_state(repository)
    real_git = release_tag.git

    def fail_tag(*args: str) -> str:
        if args[:2] == ("tag", "-a"):
            raise RuntimeError("Tag creation refused")
        return real_git(*args)

    monkeypatch.setattr(release_tag, "git", fail_tag)
    with pytest.raises(RuntimeError, match="Version commit .* remains; no tag"):
        release_tag.release("v1.0.2")
    assert git(repository, "rev-parse", "HEAD") != before[0]
    assert git(repository, "status", "--porcelain") == ""
    assert git(repository, "tag", "--list") == ""
