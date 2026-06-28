use anyhow::Context;
use std::path::Path;
use std::process::Command;

/// 检查目录是否是 Git 仓库
pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

/// 初始化 Git 仓库
pub fn git_init(path: &Path) -> anyhow::Result<()> {
    let status = Command::new("git")
        .args(["-C"])
        .arg(path)
        .args(["init"])
        .status()
        .with_context(|| format!("无法初始化 Git 仓库: {}", path.display()))?;

    if !status.success() {
        anyhow::bail!("git init 失败");
    }
    Ok(())
}

/// git add 文件或目录
pub fn git_add(repo_root: &Path, target: &Path) -> anyhow::Result<()> {
    // 计算相对路径
    let relative = target
        .strip_prefix(repo_root)
        .unwrap_or(target);

    let status = Command::new("git")
        .args(["-C"])
        .arg(repo_root)
        .args(["add"])
        .arg(relative)
        .status()
        .with_context(|| {
            format!("git add 失败: {}", relative.display())
        })?;

    if !status.success() {
        anyhow::bail!("git add 失败");
    }
    Ok(())
}

/// git rm -r 文件或目录
pub fn git_rm(repo_root: &Path, target: &Path, cached: bool) -> anyhow::Result<()> {
    let relative = target
        .strip_prefix(repo_root)
        .unwrap_or(target);

    let mut args = vec!["-C"];
    let repo_str = repo_root.display().to_string();
    args.push(&repo_str);
    args.push("rm");
    args.push("-r");

    if cached {
        args.push("--cached");
    }

    let rel_str = relative.display().to_string();
    args.push(&rel_str);

    let status = Command::new("git")
        .args(&args)
        .status()
        .with_context(|| {
            format!("git rm 失败: {}", relative.display())
        })?;

    if !status.success() {
        anyhow::bail!("git rm 失败");
    }
    Ok(())
}

/// git commit
pub fn git_commit(repo_root: &Path, message: &str) -> anyhow::Result<()> {
    let status = Command::new("git")
        .args(["-C"])
        .arg(repo_root)
        .args(["commit", "-m"])
        .arg(message)
        .status()
        .with_context(|| "git commit 失败")?;

    if !status.success() {
        anyhow::bail!("git commit 失败");
    }
    Ok(())
}

/// 从 git 根目录获取相对路径
#[allow(dead_code)]
pub fn relative_path(repo_root: &Path, full_path: &Path) -> String {
    full_path
        .strip_prefix(repo_root)
        .unwrap_or(full_path)
        .display()
        .to_string()
}
