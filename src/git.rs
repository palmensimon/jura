use anyhow::{Context, Result};
use std::process::Command;

pub fn slugify(s: &str) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    slug.split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn branch_name(ticket_key: &str, summary: &str) -> String {
    format!("{}-{}", ticket_key, slugify(summary))
}

pub fn extract_ticket_key(branch: &str) -> Option<String> {
    let bytes = branch.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if !bytes[i].is_ascii_uppercase() { i += 1; continue; }
        let start = i;
        i += 1;
        while i < len && (bytes[i].is_ascii_uppercase() || bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        if i - start < 2 || i >= len || bytes[i] != b'-' { continue; }
        i += 1;
        let num_start = i;
        while i < len && bytes[i].is_ascii_digit() { i += 1; }
        if i > num_start { return Some(branch[start..i].to_string()); }
    }
    None
}

pub fn current_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .context("Failed to run git")?;

    if !output.status.success() {
        anyhow::bail!(
            "git branch --show-current failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Returns the first local branch whose name contains `ticket_key` (case-insensitive).
pub fn find_branch_for_ticket(ticket_key: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["branch", "--list"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let key = ticket_key.to_lowercase();

    stdout.lines()
        .map(|l| l.trim().trim_start_matches("* ").to_string())
        .find(|b| b.to_lowercase().contains(&key))
}

/// Builds a browser URL for opening a new PR/MR for `branch`, derived from
/// the `origin` remote URL. Supports GitLab (SSH + HTTPS) and GitHub.
pub fn new_pr_url(branch: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let remote = String::from_utf8_lossy(&output.stdout);
    let remote = remote.trim();

    // Normalise SSH (git@host:path.git) and HTTPS to a plain https:// base URL.
    let base = if let Some(rest) = remote.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        let path = path.strip_suffix(".git").unwrap_or(path);
        format!("https://{}/{}", host, path)
    } else if remote.starts_with("https://") || remote.starts_with("http://") {
        remote.strip_suffix(".git").unwrap_or(remote).to_string()
    } else {
        return None;
    };

    if base.contains("gitlab") {
        Some(format!(
            "{}/-/merge_requests/new?merge_request%5Bsource_branch%5D={}",
            base, branch
        ))
    } else if base.contains("github") {
        Some(format!("{}/compare/{}?expand=1", base, branch))
    } else {
        None
    }
}

pub fn create_branch(branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["checkout", "-b", branch])
        .output()
        .context("Failed to run git checkout")?;

    if !output.status.success() {
        anyhow::bail!(
            "git checkout -b failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Check out `branch` if it exists locally, otherwise create and check it out.
pub fn checkout_branch(branch: &str) -> Result<()> {
    let list = Command::new("git")
        .args(["branch", "--list", branch])
        .output()
        .context("Failed to run git branch --list")?;

    let exists = !String::from_utf8_lossy(&list.stdout).trim().is_empty();

    if exists {
        let output = Command::new("git")
            .args(["checkout", branch])
            .output()
            .context("Failed to run git checkout")?;
        if !output.status.success() {
            anyhow::bail!(
                "git checkout failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        create_branch(branch)?;
    }
    Ok(())
}

/// Opens a URL in the default browser, using the appropriate command for the OS.
pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let cmd = "open";

    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";

    #[cfg(target_os = "windows")]
    let cmd = "cmd";

    #[cfg(target_os = "windows")]
    let output = Command::new(cmd)
        .args(["/C", "start", url])
        .output()
        .context("Failed to open URL")?;

    #[cfg(not(target_os = "windows"))]
    let output = Command::new(cmd)
        .arg(url)
        .output()
        .context("Failed to open URL")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to open URL: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Add primary button to home screen"), "add-primary-button-to-home-screen");
        assert_eq!(slugify("  spaces  "), "spaces");
        assert_eq!(slugify("café & résumé"), "café-résumé");
        assert_eq!(slugify("UPPERCASE"), "uppercase");
    }

    #[test]
    fn test_branch_name() {
        assert_eq!(
            branch_name("PROJ-12345", "Add primary button to home screen"),
            "PROJ-12345-add-primary-button-to-home-screen"
        );
    }

    #[test]
    fn test_extract_ticket_key() {
        assert_eq!(
            extract_ticket_key("PROJ-12345-add-primary-button"),
            Some("PROJ-12345".to_string())
        );
        assert_eq!(extract_ticket_key("main"), None);
        assert_eq!(
            extract_ticket_key("feature/PROJ-999-some-work"),
            Some("PROJ-999".to_string())
        );
    }
}
