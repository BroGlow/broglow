//! Unobtrusive app update check. One request to GitHub's latest-release
//! redirect through the system curl (as usage.rs does), once after startup
//! when allowed and on request from Settings. Failures are silent: the next
//! start or click tries again. Nothing is sent about the user or machine.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

pub const RELEASES_LATEST: &str = "https://github.com/BroGlow/broglow/releases/latest";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// A published release: its tag and page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Latest {
    pub version: String,
    pub url: String,
}

/// Ask GitHub where `releases/latest` redirects; the tag is the version.
pub fn check() -> Result<Latest> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "-sI",
        "--max-time",
        "5",
        "-o",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
        "-w",
        "%{redirect_url}",
        RELEASES_LATEST,
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    crate::spawn::configure_detached(&mut cmd);
    let out = cmd.output().context("Running curl")?;
    anyhow::ensure!(out.status.success(), "curl exited with {}", out.status);
    latest_from_redirect(std::str::from_utf8(&out.stdout)?.trim())
        .context("GitHub did not redirect to a release tag")
}

fn latest_from_redirect(url: &str) -> Option<Latest> {
    let tag = url
        .strip_prefix("https://github.com/")?
        .rsplit_once("/releases/tag/v")?
        .1;
    parse_version(tag)?;
    Some(Latest {
        version: tag.to_string(),
        url: url.to_string(),
    })
}

fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split('.').map(|part| part.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// True when `latest` is a newer release than this binary.
pub fn newer(latest: &str) -> bool {
    match (parse_version(latest), parse_version(CURRENT)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

/// Homebrew's broglow keg link, when this Mac installed BroGlow through it.
pub fn homebrew() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    ["/opt/homebrew", "/usr/local"]
        .iter()
        .map(Path::new)
        .find(|prefix| prefix.join("opt/broglow/bin/broglow").is_file())
        .map(|prefix| prefix.join("bin/brew"))
}

/// `brew update && brew upgrade broglow && broglow install`, detached:
/// `install` restarts the tray that launched it. Output goes to a log next to
/// the config. The explicit `update` matters: `upgrade` refreshes taps on its
/// own at most once a day, so right after a release it would reinstall the
/// version already present.
pub fn upgrade_with_homebrew(brew: &Path) -> Result<PathBuf> {
    let log = crate::config::config_file_path().with_file_name("update.log");
    std::fs::create_dir_all(log.parent().unwrap())?;
    let script = format!(
        "\"{brew}\" update --quiet && \"{brew}\" upgrade broglow && \"{bin}\" install",
        brew = brew.display(),
        bin = brew.with_file_name("broglow").display()
    );
    // One handle, cloned: separate descriptors keep separate offsets, so
    // stdout would overwrite what stderr already appended.
    let file = std::fs::File::create(&log)?;
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::null())
        .stdout(file.try_clone()?)
        .stderr(file);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().context("Starting the Homebrew upgrade")?;
    Ok(log)
}

/// Open a web page in the default browser.
pub fn open_url(url: &str) -> Result<()> {
    anyhow::ensure!(url.starts_with("https://github.com/"), "Unexpected URL");
    let mut cmd = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.arg(url);
        cmd
    } else if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        cmd
    } else {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };
    crate::spawn::configure_detached(&mut cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Opening the browser")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_redirect_names_the_release() {
        let latest =
            latest_from_redirect("https://github.com/BroGlow/broglow/releases/tag/v1.2.0").unwrap();
        assert_eq!(latest.version, "1.2.0");
        assert!(latest.url.ends_with("/v1.2.0"));
        for bad in [
            "",
            "https://github.com/BroGlow/broglow/releases",
            "https://github.com/BroGlow/broglow/releases/tag/nightly",
            "https://github.com/BroGlow/broglow/releases/tag/v1.2",
            "https://evil.example/releases/tag/v9.9.9",
        ] {
            assert!(latest_from_redirect(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn only_a_higher_release_counts_as_newer() {
        let (major, minor, patch) = parse_version(CURRENT).unwrap();
        assert!(newer(&format!("{major}.{minor}.{}", patch + 1)));
        assert!(newer(&format!("{}.0.0", major + 1)));
        assert!(!newer(CURRENT));
        assert!(!newer("0.9.9"));
        assert!(!newer("not-a-version"));
    }
}
