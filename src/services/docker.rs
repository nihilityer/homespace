use anyhow::Context;
use std::path::Path;
use std::process::Command;
use crate::constants::*;

/// 检查 Docker 网络是否存在
pub fn network_exists(name: &str) -> anyhow::Result<bool> {
    let output = Command::new("docker")
        .args(["network", "ls", "--format", "{{.Name}}"])
        .output()
        .context("无法执行 docker network ls")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|line| line.trim() == name))
}

/// 创建外部 Docker 网络
pub fn create_network(name: &str) -> anyhow::Result<()> {
    Command::new("docker")
        .args(["network", "create", name])
        .status()
        .with_context(|| format!("无法创建 Docker 网络: {}", name))?;
    Ok(())
}

/// 检查 Docker 容器是否存在
pub fn container_exists(name: &str) -> anyhow::Result<bool> {
    let output = Command::new("docker")
        .args(["ps", "-a", "--format", "{{.Names}}"])
        .output()
        .context("无法执行 docker ps")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|line| line.trim() == name))
}

/// 执行 docker compose up -d
pub fn compose_up(app_dir: &Path) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .args(["compose", "-f"])
        .arg(app_dir.join(COMPOSE_FILE))
        .args(["up", "-d"])
        .status()
        .with_context(|| {
            format!(
                "无法启动 docker compose: {}",
                app_dir.display()
            )
        })?;

    if !status.success() {
        anyhow::bail!("docker compose up 执行失败");
    }
    Ok(())
}

/// 执行 docker compose down
pub fn compose_down(app_dir: &Path, remove_volumes: bool) -> anyhow::Result<()> {
    let mut args = vec!["compose", "-f"];
    let compose_path = app_dir.join(COMPOSE_FILE);
    let compose_str = compose_path.display().to_string();
    args.push(&compose_str);
    args.push("down");

    if remove_volumes {
        args.push("-v");
    }

    let status = Command::new("docker")
        .args(&args)
        .status()
        .with_context(|| {
            format!("无法停止 docker compose: {}", app_dir.display())
        })?;

    if !status.success() {
        anyhow::bail!("docker compose down 执行失败");
    }
    Ok(())
}

/// 执行 docker compose stop
pub fn compose_stop(app_dir: &Path) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .args(["compose", "-f"])
        .arg(app_dir.join(COMPOSE_FILE))
        .args(["stop"])
        .status()
        .with_context(|| {
            format!("无法停止容器: {}", app_dir.display())
        })?;

    if !status.success() {
        anyhow::bail!("docker compose stop 执行失败");
    }
    Ok(())
}

/// 执行 docker compose restart
pub fn compose_restart(app_dir: &Path) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .args(["compose", "-f"])
        .arg(app_dir.join(COMPOSE_FILE))
        .args(["restart"])
        .status()
        .with_context(|| {
            format!("无法重启 docker compose: {}", app_dir.display())
        })?;

    if !status.success() {
        anyhow::bail!("docker compose restart 执行失败");
    }
    Ok(())
}

/// 执行 docker compose pull
pub fn compose_pull(app_dir: &Path) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .args(["compose", "-f"])
        .arg(app_dir.join(COMPOSE_FILE))
        .args(["pull"])
        .status()
        .with_context(|| {
            format!("无法执行 docker compose pull: {}", app_dir.display())
        })?;

    if !status.success() {
        anyhow::bail!("docker compose pull 执行失败");
    }
    Ok(())
}

/// 执行 docker compose build
pub fn compose_build(app_dir: &Path) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .args(["compose", "-f"])
        .arg(app_dir.join(COMPOSE_FILE))
        .args(["build"])
        .status()
        .with_context(|| {
            format!("无法执行 docker compose build: {}", app_dir.display())
        })?;

    if !status.success() {
        anyhow::bail!("docker compose build 执行失败");
    }
    Ok(())
}

/// 执行 docker compose ps (JSON)
#[allow(dead_code)]
pub fn compose_ps_json(app_dir: &Path) -> anyhow::Result<String> {
    let output = Command::new("docker")
        .args(["compose", "-f"])
        .arg(app_dir.join(COMPOSE_FILE))
        .args(["ps", "--format", "json"])
        .output()
        .with_context(|| {
            format!("无法查询容器状态: {}", app_dir.display())
        })?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// 获取 Traefik /ping 响应
pub fn traefik_ping() -> anyhow::Result<bool> {
    // 通过 docker exec 检查 traefik 服务
    let output = Command::new("docker")
        .args([
            "exec",
            CONTAINER_TRAEFIK,
            "wget",
            "-qO-",
            "http://localhost:8080/ping",
        ])
        .output();

    match output {
        Ok(out) => Ok(out.status.success()),
        Err(_) => Ok(false),
    }
}
