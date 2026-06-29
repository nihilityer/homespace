use tracing::info;

use crate::services::docker;
use crate::services::scanner;
use crate::config::Config;
use crate::constants::*;
use dialoguer::{Confirm, Input};

/// 升级应用镜像版本。
///
/// 更新 `.env` 中 `{SERVICE}_VERSION` 的值，然后执行 `docker compose pull && up -d`。
pub fn run(app_name: &str, service_name: Option<&str>, config: &Config) -> anyhow::Result<()> {
    let apps = scanner::scan_apps(config)?;
    let app = apps
        .iter()
        .find(|a| a.name == app_name)
        .ok_or_else(|| anyhow::anyhow!("未找到应用: {app_name}"))?;

    // 选择要升级的服务
    let targets: Vec<&crate::models::app::Service> = if let Some(svc_name) = service_name {
        let svc = app
            .services
            .iter()
            .find(|s| s.name == svc_name)
            .ok_or_else(|| anyhow::anyhow!("未找到服务: {svc_name}"))?;
        vec![svc]
    } else if app.services.len() == 1 {
        vec![&app.services[0]]
    } else {
        // 多服务应用：全部升级
        app.services.iter().collect()
    };

    info!("⬆️  升级应用: {}\n", app.name);
    for svc in &targets {
        info!("  服务: {}  当前镜像: {}:{}", svc.name, svc.image, svc.version);
    }

    // 确认
    let confirmed = Confirm::new()
        .with_prompt("\n确认升级?")
        .default(true)
        .interact()?;
    if !confirmed {
        info!("已取消");
        return Ok(());
    }

    // 收集新版本
    let mut updates: Vec<(&str, String)> = Vec::new();
    for svc in &targets {
        let new_version: String = Input::new()
            .with_prompt(format!("{} 新版本 (当前: {})", svc.name, svc.version))
            .with_initial_text(svc.version.clone())
            .interact_text()?;
        if new_version != svc.version {
            updates.push((&svc.name, new_version));
        }
    }

    if updates.is_empty() {
        info!("版本未变更，跳过");
        return Ok(());
    }

    // 更新 .env 文件
    let env_path = app.path.join(ENV_FILE);
    let mut content = std::fs::read_to_string(&env_path)
        .ok()
        .unwrap_or_default();

    for (svc_name, new_version) in &updates {
        let var_prefix = svc_name.to_uppercase().replace('-', "_");
        let key = format!("{}_VERSION=", var_prefix);

        let mut found = false;
        let new_lines: Vec<String> = content
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with(&key) && !trimmed.starts_with('#') {
                    found = true;
                    format!("{}={}", var_prefix, new_version)
                } else {
                    line.to_string()
                }
            })
            .collect();

        if found {
            content = new_lines.join("\n") + "\n";
        } else {
            // 没找到 VERSION 行，追加
            content.push_str(&format!("{}_VERSION={}\n", var_prefix, new_version));
        }
    }

    std::fs::write(&env_path, &content)?;
    info!("\n✅ .env 已更新");

    // Pull + Up
    info!("\n⬇️  拉取新镜像...");
    docker::compose_pull(&app.path)?;

    info!("🚀 重新创建容器...");
    docker::compose_up(&app.path)?;

    info!("\n✅ {} 升级完成", app.name);
    for (svc_name, new_version) in &updates {
        info!("  {} → {}", svc_name, new_version);
    }

    Ok(())
}
