use tracing::info;
use crate::config::Config;
use crate::models::app::DatabaseConfig;
use crate::services::database;
use crate::services::docker;
use crate::services::git;
use crate::services::scanner;
use anyhow::Context;
use dialoguer::{Confirm, Select};
use crate::constants::*;

/// 移除应用（停止容器、清理数据、Git 操作）
pub fn run(
    app_name: &str,
    config: &Config,
    force: bool,
    purge: bool,
    no_git: bool,
    commit: bool,
) -> anyhow::Result<()> {
    let apps = scanner::scan_apps(config)?;
    let app = apps
        .iter()
        .find(|a| a.name == app_name)
        .ok_or_else(|| anyhow::anyhow!("未找到应用: {}", app_name))?;

    // 保护基础设施
    if app.name == TRAEFIK_DIR || app.name == POSTGRES_DIR {
        let confirm = Confirm::new()
            .with_prompt(format!("⚠️  你正在尝试移除 {} 基础设施！确认继续?", app.name))
            .default(false)
            .interact()?;
        if !confirm {
            info!("已取消");
            return Ok(());
        }
    }

    info!("🗑️  homespace - 移除应用: {}\n", app.name);

    if force {
        // force 模式：直接 docker compose down
        docker::compose_down(&app.path, purge)?;
        if purge {
            let data_dir = app.data_root.as_ref().unwrap_or(&config.paths.data_root);
            let full_data = if data_dir.is_relative() {
                app.path.join(data_dir).join(&app.name)
            } else {
                data_dir.join(&app.name)
            };
            if full_data.exists() {
                std::fs::remove_dir_all(&full_data)
                    .with_context(|| format!("无法删除数据目录: {}", full_data.display()))?;
            }
        }
    } else {
        let options = &[
            "仅停止容器 (docker compose stop)",
            "停止并删除容器，保留数据 (docker compose down)",
            "停止、删除容器并清理数据目录 (docker compose down -v + rm -rf)",
        ];
        let selection = Select::new()
            .with_prompt("请选择操作")
            .items(options)
            .default(1)
            .interact()?;

        let action = match selection {
            0 => "stop",
            1 => "down",
            2 => "purge",
            _ => "down",
        };

        let confirmed = Confirm::new()
            .with_prompt(format!(
                "确认对 {} 执行 {} 操作?",
                app.name, options[selection]
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            info!("已取消");
            return Ok(());
        }

        match action {
            "stop" => docker::compose_stop(&app.path)?,
            "down" => docker::compose_down(&app.path, false)?,
            "purge" => {
                docker::compose_down(&app.path, true)?;
                // 删除数据目录
                let data_dir = app
                    .data_root
                    .as_ref()
                    .map(|p| {
                        if p.is_relative() {
                            app.path.join(p).join(&app.name)
                        } else {
                            p.join(&app.name)
                        }
                    })
                    .unwrap_or_else(|| config.paths.data_root.join(&app.name));

                if data_dir.exists() {
                    std::fs::remove_dir_all(&data_dir)
                        .with_context(|| format!("无法删除数据目录: {}", data_dir.display()))?;
                    info!("  ✅ 数据目录已删除: {}", data_dir.display());
                }
            }
            _ => anyhow::bail!("内部错误: 未知的操作选项"),
        }
    }

    // 数据库操作
    for svc in &app.services {
        if let DatabaseConfig::SharedPostgres {
            db_name, user, ..
        } = &svc.database
        {
            let drop_db = if force {
                true
            } else {
                Confirm::new()
                    .with_prompt(format!(
                        "该应用使用了 infra 共享 PostgreSQL (DB: {}, User: {})，是否同时删除?",
                        db_name, user
                    ))
                    .default(false)
                    .interact()?
            };

            if drop_db {
                let admin_user =
                    database::read_infra_pg_user(&config.postgres_env_path()).unwrap_or_else(|_| {
                        "admin".to_string()
                    });
                let admin_password =
                    database::read_infra_pg_password(&config.postgres_env_path())
                        .unwrap_or_default();
                let admin_db = database::read_infra_pg_db(&config.postgres_env_path());

                if !admin_password.is_empty() {
                    database::drop_database_and_user(
                        db_name,
                        user,
                        NETWORK_POSTGRES,
                        &admin_user,
                        &admin_password,
                        &admin_db,
                    )?;
                    info!("  ✅ 数据库 {} 和用户 {} 已删除", db_name, user);
                }
            }
        }
    }

    // Git 操作
    if !no_git && git::is_git_repo(&config.paths.apps_root) {
        git::git_rm(&config.paths.apps_root, &app.path, !purge)?;
        info!("  ✅ git rm {}", app.name);

        if commit {
            git::git_commit(
                &config.paths.apps_root,
                &format!("homespace: remove {}", app.name),
            )?;
            info!("  ✅ git commit");
        }
    }

    info!("\n✅ 应用 {} 已移除", app.name);

    Ok(())
}
