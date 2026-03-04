use crate::cli::ServiceAction;
use crate::service;
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use anyhow::bail;
#[cfg(target_os = "windows")]
use std::process::Command;

const PATH_BLOCK_START: &str = "# >>> sysray path >>>";
const PATH_BLOCK_END: &str = "# <<< sysray path <<<";

pub async fn install_current_executable(
    install_service: bool,
    install_path_entry: bool,
) -> Result<()> {
    let source = env::current_exe().context("Failed to resolve current executable path")?;
    let destination = install_path()?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    if source != destination {
        fs::copy(&source, &destination).with_context(|| {
            format!(
                "Failed to copy executable from {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        ensure_executable(&destination)?;
    }

    println!("Installed binary: {}", destination.display());

    if install_path_entry {
        ensure_path_registration(destination.parent().unwrap_or(Path::new("")))?;
    }

    if install_service {
        service::run_service_with_exe(ServiceAction::Install, Some(destination.as_path())).await?;
    }

    if !path_contains(destination.parent().unwrap_or(Path::new(""))) {
        println!(
            "Current session PATH does not yet include {}",
            destination.parent().unwrap_or(Path::new("")).display()
        );
        print_current_session_hint(destination.parent().unwrap_or(Path::new("")));
    }

    Ok(())
}

pub fn install_path() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let local_app_data = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("LOCALAPPDATA is not set")?;
        Ok(local_app_data
            .join("Programs")
            .join("Sysray")
            .join("sysray.exe"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        Ok(home.join(".local").join("bin").join("sysray"))
    }
}

fn path_contains(dir: &Path) -> bool {
    env::var_os("PATH")
        .map(|value| env::split_paths(&value).any(|entry| entry == dir))
        .unwrap_or(false)
}

fn ensure_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

fn ensure_path_registration(dir: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::ensure_user_path(dir)?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        unix::ensure_user_path(dir)?;
    }

    if path_contains(dir) {
        println!("Current session PATH already contains {}", dir.display());
    }

    Ok(())
}

fn print_current_session_hint(dir: &Path) {
    #[cfg(target_os = "windows")]
    {
        println!(
            "Open a new Command Prompt / PowerShell window to pick up {}",
            dir.display()
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!(
            "Open a new shell or run: export PATH=\"{}:$PATH\"",
            dir.display()
        );
    }
}

#[cfg(not(target_os = "windows"))]
mod unix {
    use super::*;

    pub fn ensure_user_path(dir: &Path) -> Result<()> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        let shell = env::var("SHELL").unwrap_or_default();
        let mut updated_files = Vec::new();

        // Login shells and desktop sessions usually consume this.
        for file in [
            home.join(".profile"),
            home.join(".bash_profile"),
            home.join(".bashrc"),
            home.join(".zshrc"),
            home.join(".zprofile"),
        ] {
            if should_manage_unix_file(&file, &shell) && update_unix_path_file(&file, dir)? {
                updated_files.push(file);
            }
        }

        let fish_conf = home.join(".config/fish/conf.d/sysray_path.fish");
        if should_manage_fish(&fish_conf, &shell) && update_fish_path_file(&fish_conf, dir)? {
            updated_files.push(fish_conf);
        }

        if updated_files.is_empty() {
            println!(
                "PATH registration already present for future shells: {}",
                dir.display()
            );
        } else {
            println!("Persisted PATH entry for future shells: {}", dir.display());
            for file in updated_files {
                println!("Updated shell profile: {}", file.display());
            }
        }

        Ok(())
    }

    fn should_manage_unix_file(path: &Path, shell: &str) -> bool {
        if path.exists() {
            return true;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            return false;
        };

        match name {
            ".profile" => true,
            ".bash_profile" | ".bashrc" => shell.contains("bash"),
            ".zshrc" | ".zprofile" => shell.contains("zsh"),
            _ => false,
        }
    }

    fn should_manage_fish(path: &Path, shell: &str) -> bool {
        path.exists() || shell.contains("fish")
    }

    fn update_unix_path_file(path: &Path, dir: &Path) -> Result<bool> {
        let snippet = unix_path_block(dir);
        update_text_file(path, &snippet)
    }

    fn update_fish_path_file(path: &Path, dir: &Path) -> Result<bool> {
        let snippet = fish_path_block(dir);
        update_text_file(path, &snippet)
    }

    fn update_text_file(path: &Path, block: &str) -> Result<bool> {
        let content = fs::read_to_string(path).unwrap_or_default();
        let updated = upsert_managed_block(&content, block);
        if updated == content {
            return Ok(false);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, updated)?;
        Ok(true)
    }

    fn unix_path_block(dir: &Path) -> String {
        format!(
            "{PATH_BLOCK_START}\nexport PATH=\"{}:$PATH\"\n{PATH_BLOCK_END}\n",
            dir.display()
        )
    }

    fn fish_path_block(dir: &Path) -> String {
        format!(
            "{PATH_BLOCK_START}\nif not contains \"{}\" $PATH\n    set -gx PATH \"{}\" $PATH\nend\n{PATH_BLOCK_END}\n",
            dir.display(),
            dir.display()
        )
    }

    fn upsert_managed_block(content: &str, block: &str) -> String {
        match (content.find(PATH_BLOCK_START), content.find(PATH_BLOCK_END)) {
            (Some(start), Some(end)) if start <= end => {
                let after_end = end + PATH_BLOCK_END.len();
                let suffix = content[after_end..]
                    .strip_prefix('\n')
                    .unwrap_or(&content[after_end..]);
                let prefix = content[..start].trim_end_matches('\n');
                if prefix.is_empty() {
                    format!("{block}{}", suffix_if_needed(suffix))
                } else if suffix.is_empty() {
                    format!("{prefix}\n{block}")
                } else {
                    format!("{prefix}\n{block}\n{suffix}")
                }
            }
            _ => {
                if content.trim().is_empty() {
                    block.to_string()
                } else {
                    format!("{}\n{block}", content.trim_end_matches('\n'))
                }
            }
        }
    }

    fn suffix_if_needed(suffix: &str) -> &str {
        if suffix.is_empty() {
            ""
        } else {
            suffix
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn upsert_appends_managed_block_when_missing() {
            let updated = upsert_managed_block(
                "export FOO=bar\n",
                "# >>> sysray path >>>\nexport PATH=\"/tmp:$PATH\"\n# <<< sysray path <<<\n",
            );
            assert!(updated.contains("export FOO=bar"));
            assert!(updated.contains("export PATH=\"/tmp:$PATH\""));
        }

        #[test]
        fn upsert_replaces_existing_managed_block() {
            let content = "# >>> sysray path >>>\nold\n# <<< sysray path <<<\nexport FOO=bar\n";
            let updated = upsert_managed_block(
                content,
                "# >>> sysray path >>>\nexport PATH=\"/new:$PATH\"\n# <<< sysray path <<<\n",
            );
            assert!(!updated.contains("\nold\n"));
            assert!(updated.contains("export PATH=\"/new:$PATH\""));
            assert!(updated.contains("export FOO=bar"));
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    pub fn ensure_user_path(dir: &Path) -> Result<()> {
        let dir = dir.to_string_lossy();
        let script = r#"
$entry = [System.IO.Path]::GetFullPath($args[0])
$current = [Environment]::GetEnvironmentVariable('Path', 'User')
$parts = @()
if ($current) {
    $parts = $current -split ';' | Where-Object { $_ -and $_.Trim() -ne '' }
}
$normalized = @()
foreach ($part in $parts) {
    try {
        $normalized += [System.IO.Path]::GetFullPath($part)
    } catch {
        $normalized += $part
    }
}
if ($normalized -contains $entry) {
    exit 0
}
$newPath = if ([string]::IsNullOrWhiteSpace($current)) { $entry } else { "$current;$entry" }
[Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
"#;

        let status = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
                &dir,
            ])
            .status()
            .context("Failed to update the user PATH through PowerShell")?;

        if !status.success() {
            bail!("Failed to persist {} in the user PATH", dir);
        }

        println!("Persisted PATH entry for future sessions: {}", dir);
        Ok(())
    }
}
