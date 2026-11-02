"""Upload a checked static artifact and atomically activate it on the VPS."""

import argparse
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile
from urllib.parse import quote
import uuid

ROOT = Path("/srv/broglow")
RELEASE = re.compile(r"[0-9a-f]{40}-[0-9]+-[0-9]+")


def validate_artifact(artifact):
    for name in ("index.html", "404.html", "docs/index.html", "docs/ru/index.html"):
        if not (artifact / name).is_file():
            raise ValueError(f"Incomplete website: missing {name}")
    for path in (artifact, *artifact.rglob("*")):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise ValueError(f"Website contains a symlink or special file: {path}")


def activate(release, root=ROOT, rollback=False):
    if not RELEASE.fullmatch(release):
        raise ValueError("Invalid release identity")
    destination = root / "releases" / release
    source = destination if rollback else root / f"incoming-{release}"
    if root.is_symlink() or (root / "releases").is_symlink():
        raise ValueError("Website root and releases must be real directories")
    validate_artifact(source)
    for path in (source, *source.rglob("*")):
        path.chmod(0o755 if path.is_dir() else 0o644)
    if not rollback:
        if destination.exists() or destination.is_symlink():
            raise ValueError("Release already exists; use another workflow attempt")
        source.rename(destination)
    temporary = root / f".current-{uuid.uuid4().hex}"
    try:
        temporary.symlink_to(Path("releases") / release, target_is_directory=True)
        os.replace(temporary, root / "current")
    finally:
        temporary.unlink(missing_ok=True)


def latest_eligible_run():
    """Unrelated commits do not supersede a path-filtered Website deployment."""
    branch = quote(os.environ["GITHUB_DEFAULT_BRANCH"], safe="")
    repository = os.environ["GITHUB_REPOSITORY"]
    number = int(os.environ["GITHUB_RUN_NUMBER"])
    for event in ("push", "workflow_dispatch"):
        endpoint = (
            f"repos/{repository}/actions/workflows/pages.yaml/runs"
            f"?branch={branch}&event={event}&per_page=1"
        )
        result = subprocess.run(
            ["gh", "api", endpoint], check=True, capture_output=True, text=True
        )
        if any(
            run["run_number"] > number
            for run in json.loads(result.stdout)["workflow_runs"]
        ):
            return False
    return True


def upload(artifact):
    if os.environ.get("VPS_DEPLOY_ENABLED") != "true":
        raise ValueError("VPS deployment is disabled")
    host = os.environ["VPS_HOST"]
    user = os.environ["VPS_USER"]
    port = os.environ.get("VPS_PORT") or "22"
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9.:-]*", host):
        raise ValueError("VPS_HOST must be a hostname or IP address")
    if not re.fullmatch(r"[a-z_][a-z0-9_-]*", user) or user == "root":
        raise ValueError("VPS_USER must be a dedicated non-root account")
    if not port.isdecimal() or not 1 <= int(port) <= 65535:
        raise ValueError("Invalid VPS_PORT")
    release = "-".join(
        os.environ[name]
        for name in ("GITHUB_SHA", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT")
    )
    if not RELEASE.fullmatch(release):
        raise ValueError("Invalid workflow release identity")
    key = os.environ["VPS_SSH_KEY"]
    known_hosts = os.environ["VPS_KNOWN_HOSTS"]
    if not key.strip() or not known_hosts.strip():
        raise ValueError("Dedicated SSH key and trusted known_hosts entry are required")
    validate_artifact(artifact)
    if not latest_eligible_run():
        print("Skipping superseded Website run; no VPS connection made")
        return
    with tempfile.TemporaryDirectory(prefix="broglow-ssh-") as temporary:
        credentials = Path(temporary)
        private_key = credentials / "key"
        private_key.write_text(key.rstrip() + "\n")
        private_key.chmod(0o600)
        hosts = credentials / "known_hosts"
        hosts.write_text(known_hosts.rstrip() + "\n")
        ssh = [
            "ssh",
            "-F",
            "/dev/null",
            "-o",
            "BatchMode=yes",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            f"UserKnownHostsFile={hosts}",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "KbdInteractiveAuthentication=no",
            "-o",
            "ConnectTimeout=15",
            "-i",
            str(private_key),
            "-p",
            port,
            "-l",
            user,
            "--",
            host,
        ]
        incoming = ROOT / f"incoming-{release}"
        command = (
            f"set -eu; umask 022; mkdir -- {shlex.quote(str(incoming))}; "
            f"tar --no-same-owner --no-same-permissions -xf - -C {shlex.quote(str(incoming))}"
        )
        with subprocess.Popen(
            ["tar", "-C", str(artifact), "-cf", "-", "."], stdout=subprocess.PIPE
        ) as archive:
            try:
                subprocess.run(ssh + [command], stdin=archive.stdout, check=True)
            finally:
                archive.stdout.close()
            if archive.wait() != 0:
                raise ValueError("Website archive upload failed")
        if not latest_eligible_run():
            print("Skipping activation: a newer eligible Website run exists")
            return
        subprocess.run(
            ssh + [shlex.join(["python3", "-", "--activate", release])],
            input=Path(__file__).read_bytes(),
            check=True,
        )
    print(f"Activated website release {release}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", nargs="?", type=Path)
    operation = parser.add_mutually_exclusive_group()
    operation.add_argument("--activate", metavar="RELEASE")
    operation.add_argument("--rollback", metavar="RELEASE")
    args = parser.parse_args()
    try:
        if args.activate or args.rollback:
            if args.artifact:
                parser.error("Artifact cannot be combined with server activation")
            activate(args.activate or args.rollback, rollback=bool(args.rollback))
        elif args.artifact:
            upload(args.artifact)
        else:
            parser.error("Provide the checked artifact directory")
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Website deployment failed: {error}\n")


if __name__ == "__main__":
    main()
