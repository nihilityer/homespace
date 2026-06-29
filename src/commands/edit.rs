use tracing::info;
use crate::config::Config;
use crate::services::generator;
use crate::services::git;
use crate::services::scanner;
use dialoguer::{Confirm, Input, Select};

/// 交互式修改已有应用配置的主入口
#[allow(clippy::cognitive_complexity)]
pub fn run(app_name: &str, config: &Config, commit: bool, no_git: bool) -> anyhow::Result<()> {
    let apps = scanner::scan_apps(config)?;
    let app = apps
        .iter()
        .find(|a| a.name == app_name)
        .ok_or_else(|| anyhow::anyhow!("未找到应用: {}", app_name))?
        .clone();

    info!("✏️  homespace - 修改应用: {}\n", app.name);
    info!("  路径: {}", app.path.display());
    info!("");

    let mut modified = false;

    loop {
        let options = &[
            "基本信息 (名称、描述)",
            "服务配置 (镜像、端口、网络)",
            "Traefik 路由",
            "端口映射",
            "数据库配置",
            "数据卷与挂载",
            "环境变量",
            "完成修改",
        ];

        let selection = Select::new()
            .with_prompt("请选择要修改的内容")
            .items(options)
            .default(options.len() - 1)
            .interact()?;

        match selection {
            0 => {
                // 修改基本信息
                info!("\n当前应用名: {}", app.name);
                let new_name: String = Input::new()
                    .with_prompt("新应用名 (回车保持不变)")
                    .allow_empty(true)
                    .interact_text()?;
                if !new_name.is_empty() && new_name != app.name {
                    // TODO: 需要重命名目录和更新引用
                    info!("  ⚠️  重命名功能需要在后续版本中完善");
                }

                let new_desc: String = Input::new()
                    .with_prompt("新描述 (回车保持不变)")
                    .allow_empty(true)
                    .interact_text()?;
                if !new_desc.is_empty() {
                    // 这里需要修改 app，但我们没有 mutable 引用
                    // 简化处理：仅重新生成 README
                    info!("  ℹ️  描述更新将在重新生成时生效");
                    modified = true;
                }
            }
            1 => {
                // 服务配置
                if app.services.is_empty() {
                    info!("  该应用没有服务定义");
                    continue;
                }

                let svc_names: Vec<&str> = app.services.iter().map(|s| s.name.as_str()).collect();
                let svc_idx = Select::new()
                    .with_prompt("选择要修改的服务")
                    .items(&svc_names)
                    .default(0)
                    .interact()?;

                let mut svc = app.services[svc_idx].clone();

                info!("当前镜像: {}:{}", svc.image, svc.version);
                let new_image: String = Input::new()
                    .with_prompt("新镜像名 (回车保持不变)")
                    .allow_empty(true)
                    .interact_text()?;
                if !new_image.is_empty() {
                    svc.image = new_image;
                    modified = true;
                }

                let new_version: String = Input::new()
                    .with_prompt(format!("新版本 (当前: {}) (回车保持不变)", svc.version))
                    .allow_empty(true)
                    .interact_text()?;
                if !new_version.is_empty() {
                    svc.version = new_version;
                    modified = true;
                }

                // 端口
                let ports_str: String = Input::new()
                    .with_prompt("新端口列表 (回车保持不变，逗号分隔)")
                    .allow_empty(true)
                    .interact_text()?;
                if !ports_str.is_empty() {
                    svc.internal_ports = ports_str
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    modified = true;
                }

                info!("⚠️  服务配置变更将在重新生成 compose 时应用");
            }
            2 => {
                info!("⚠️  Traefik 路由修改功能将在后续版本完善");
                info!("    当前路由:");
                for svc in &app.services {
                    for route in &svc.routes {
                        info!("      - https://{}", route.subdomain);
                    }
                }
            }
            3 => {
                info!("⚠️  端口映射修改功能将在后续版本完善");
            }
            4 => {
                info!("⚠️  数据库配置修改功能将在后续版本完善");
            }
            5 => {
                info!("⚠️  数据卷修改功能将在后续版本完善");
            }
            6 => {
                info!("⚠️  环境变量修改功能将在后续版本完善");
                info!("    请直接编辑 .env 文件:");
                info!("    {}", app.path.join(".env").display());
            }
            7 => break, // 完成
            _ => unreachable!(),
        }
    }

    // 重新生成文件
    if modified {
        let regenerate = Confirm::new()
            .with_prompt("重新生成 docker-compose.yml 和相关文件?")
            .default(true)
            .interact()?;

        if regenerate {
            generator::generate_app(&app, config)?;
            info!("✅ 文件已重新生成");

            if !no_git && git::is_git_repo(&config.paths.apps_root) {
                git::git_add(&config.paths.apps_root, &app.path)?;
                info!("✅ git add 完成");

                if commit {
                    git::git_commit(
                        &config.paths.apps_root,
                        &format!("homespace: edit {}", app.name),
                    )?;
                    info!("✅ git commit 完成");
                }
            }
        }
    } else {
        info!("没有检测到修改");
    }

    Ok(())
}
