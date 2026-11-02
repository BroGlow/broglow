"""Exercise deployment locally; SSH is replaced and no server is contacted."""

import json
import os
from pathlib import Path
import subprocess
import sys

import pytest

import deploy

FIRST = "a" * 40 + "-123-1"
SECOND = "b" * 40 + "-124-1"


def artifact(path: Path) -> Path:
    for name in ("index.html", "404.html", "docs/index.html", "docs/ru/index.html"):
        target = path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(name)
    return path


def test_activation_and_rollback_replace_only_current_link(tmp_path: Path) -> None:
    (tmp_path / "releases").mkdir()
    first = artifact(tmp_path / f"incoming-{FIRST}")

    deploy.activate(FIRST, root=tmp_path)

    assert not first.exists()
    assert (tmp_path / "current").readlink() == Path("releases") / FIRST
    artifact(tmp_path / f"incoming-{SECOND}")
    deploy.activate(SECOND, root=tmp_path)
    assert (tmp_path / "current").readlink() == Path("releases") / SECOND
    assert (tmp_path / "releases" / FIRST / "index.html").is_file()

    deploy.activate(FIRST, root=tmp_path, rollback=True)

    assert (tmp_path / "current").readlink() == Path("releases") / FIRST
    assert (tmp_path / "releases" / SECOND / "index.html").is_file()
    assert not list(tmp_path.glob(".current-*"))


@pytest.mark.parametrize("broken", ("missing", "symlink", "special"))
def test_invalid_upload_preserves_live_release(tmp_path: Path, broken: str) -> None:
    (tmp_path / "releases").mkdir()
    artifact(tmp_path / f"incoming-{FIRST}")
    deploy.activate(FIRST, root=tmp_path)
    incoming = artifact(tmp_path / f"incoming-{SECOND}")
    if broken == "missing":
        (incoming / "docs/ru/index.html").unlink()
    elif broken == "symlink":
        (incoming / "private").symlink_to(tmp_path)
    else:
        os.mkfifo(incoming / "pipe")

    with pytest.raises(ValueError, match="Incomplete website|symlink or special"):
        deploy.activate(SECOND, root=tmp_path)

    assert (tmp_path / "current").readlink() == Path("releases") / FIRST
    assert not (tmp_path / "releases" / SECOND).exists()


def test_invalid_release_cannot_escape_website_root(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="Invalid release identity"):
        deploy.activate("../../elsewhere", root=tmp_path)
    assert not list(tmp_path.iterdir())


def test_failed_activation_keeps_current_release(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "releases").mkdir()
    artifact(tmp_path / f"incoming-{FIRST}")
    deploy.activate(FIRST, root=tmp_path)
    artifact(tmp_path / f"incoming-{SECOND}")

    def fail_replace(source: Path, destination: Path) -> None:
        raise OSError("Cannot replace current link")

    monkeypatch.setattr(deploy.os, "replace", fail_replace)

    with pytest.raises(OSError, match="Cannot replace current"):
        deploy.activate(SECOND, root=tmp_path)

    assert (tmp_path / "current").readlink() == Path("releases") / FIRST
    assert (tmp_path / "releases" / SECOND / "index.html").is_file()
    assert not list(tmp_path.glob(".current-*"))


@pytest.fixture
def upload_environment(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    executable = tmp_path / "ssh"
    executable.write_text(
        f"#!{sys.executable}\n"
        "import json, os, sys\n"
        "from pathlib import Path\n"
        "payload = sys.stdin.buffer.read()\n"
        "with open(os.environ['SSH_LOG'], 'a') as log:\n"
        "    log.write(json.dumps({'args': sys.argv[1:], 'bytes': len(payload)}) + '\\n')\n"
        "if sys.argv[-1].startswith('python3 '):\n"
        "    compile(payload, '<activation>', 'exec')\n"
        "sys.exit(int(os.environ.get('SSH_FAILURE', '0')))\n"
    )
    executable.chmod(0o755)
    monkeypatch.setenv("PATH", f"{tmp_path}:{os.environ['PATH']}")
    for name, value in {
        "VPS_DEPLOY_ENABLED": "true",
        "VPS_HOST": "host.example",
        "VPS_USER": "broglow-deploy",
        "VPS_PORT": "2222",
        "VPS_SSH_KEY": "test-key",
        "VPS_KNOWN_HOSTS": "[host.example]:2222 ssh-ed25519 test-key",
        "GITHUB_SHA": "a" * 40,
        "GITHUB_RUN_ID": "123",
        "GITHUB_RUN_ATTEMPT": "1",
        "SSH_LOG": str(tmp_path / "ssh.log"),
    }.items():
        monkeypatch.setenv(name, value)
    monkeypatch.setattr(deploy, "latest_eligible_run", lambda: True)
    return artifact(tmp_path / "artifact")


def ssh_calls(artifact: Path) -> list[dict]:
    log = artifact.parent / "ssh.log"
    return [json.loads(line) for line in log.read_text().splitlines()]


def test_upload_uses_pinned_ssh_identity_and_activates_after_transfer(
    upload_environment: Path,
) -> None:
    deploy.upload(upload_environment)

    calls = ssh_calls(upload_environment)
    assert len(calls) == 2
    options = calls[0]["args"]
    assert "StrictHostKeyChecking=yes" in options
    assert "IdentitiesOnly=yes" in options
    assert options[options.index("-p") + 1] == "2222"
    assert options[options.index("-l") + 1] == "broglow-deploy"
    assert options[-2] == "host.example"
    assert not Path(options[options.index("-i") + 1]).exists()
    assert calls[0]["bytes"] > 0
    assert calls[1]["args"][-1] == f"python3 - --activate {FIRST}"


def test_failed_upload_does_not_activate(
    upload_environment: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("SSH_FAILURE", "1")

    with pytest.raises(subprocess.CalledProcessError):
        deploy.upload(upload_environment)

    assert len(ssh_calls(upload_environment)) == 1


def test_new_eligible_website_run_prevents_activation(
    upload_environment: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    revisions = iter((True, False))
    monkeypatch.setattr(deploy, "latest_eligible_run", lambda: next(revisions))

    deploy.upload(upload_environment)

    assert len(ssh_calls(upload_environment)) == 1


def test_superseded_run_makes_no_server_connection(
    upload_environment: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(deploy, "latest_eligible_run", lambda: False)

    deploy.upload(upload_environment)

    assert not (upload_environment.parent / "ssh.log").exists()


def test_disabled_deployment_makes_no_server_connection(
    upload_environment: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("VPS_DEPLOY_ENABLED")

    with pytest.raises(ValueError, match="deployment is disabled"):
        deploy.upload(upload_environment)

    assert not (upload_environment.parent / "ssh.log").exists()


@pytest.mark.parametrize(
    ("name", "value"),
    (("VPS_HOST", "-oProxyCommand=bad"), ("VPS_USER", "root"), ("VPS_PORT", "70000")),
)
def test_invalid_ssh_settings_make_no_server_connection(
    upload_environment: Path,
    monkeypatch: pytest.MonkeyPatch,
    name: str,
    value: str,
) -> None:
    monkeypatch.setenv(name, value)

    with pytest.raises(ValueError):
        deploy.upload(upload_environment)

    assert not (upload_environment.parent / "ssh.log").exists()


@pytest.mark.parametrize(
    ("newer_event", "expected"),
    ((None, True), ("push", False), ("workflow_dispatch", False)),
)
def test_only_newer_eligible_website_runs_supersede_deployment(
    monkeypatch: pytest.MonkeyPatch, newer_event: str | None, expected: bool
) -> None:
    for name, value in {
        "GITHUB_DEFAULT_BRANCH": "master",
        "GITHUB_REPOSITORY": "broglow/broglow",
        "GITHUB_RUN_NUMBER": "10",
        # HEAD can have advanced because Rust-only pushes do not run this workflow.
        "GITHUB_SHA": "a" * 40,
    }.items():
        monkeypatch.setenv(name, value)
    requests = []

    def gh_api(command: list[str], **kwargs) -> subprocess.CompletedProcess:
        endpoint = command[2]
        requests.append(endpoint)
        assert command[:2] == ["gh", "api"]
        assert "/actions/workflows/pages.yaml/runs?branch=master&" in endpoint
        assert "per_page=1" in endpoint
        event = "workflow_dispatch" if "event=workflow_dispatch" in endpoint else "push"
        return subprocess.CompletedProcess(
            command,
            0,
            stdout=json.dumps(
                {
                    "workflow_runs": [
                        {
                            "run_number": 11 if event == newer_event else 10,
                            "head_sha": "b" * 40,
                            "event": event,
                        }
                    ]
                }
            ),
        )

    monkeypatch.setattr(deploy.subprocess, "run", gh_api)

    assert deploy.latest_eligible_run() is expected
    assert requests
    assert all(
        "event=push" in url or "event=workflow_dispatch" in url for url in requests
    )
