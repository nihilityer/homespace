use tracing::info;
use crate::config::Config;
use crate::services::scanner;

/// 查看应用详细配置与运行状态
#[allow(clippy::cognitive_complexity)]
pub fn run(app_name: &str, config: &Config) -> anyhow::Result<()> {
    let apps = scanner::scan_apps(config)?;
    let app = apps
        .iter()
        .find(|a| a.name == app_name)
        .ok_or_else(|| anyhow::anyhow!("未找到应用: {}", app_name))?;

    let statuses = scanner::get_app_status(app).unwrap_or_default();

    info!("📦 {} — {}\n", app.name, app.description);
    info!("  路径: {}", app.path.display());
    info!(
        "  数据根: {}",
        app.data_root
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "全局默认".to_string())
    );
    info!("  服务数: {}", app.services.len());
    info!("");

    for (i, svc) in app.services.iter().enumerate() {
        info!("  ┌─ Service: {}", svc.name);
        info!("  │  镜像: {}", svc.image);

        // 启动命令
        if let Some(ref cmd) = svc.command {
            info!("  │  命令: {}", cmd.join(" "));
        }

        // 状态
        let svc_status = statuses.iter().find(|s| s.name == svc.name);
        let state_str = match svc_status {
            Some(s) => format!("{:?}", s.state),
            None => "unknown".to_string(),
        };
        info!("  │  状态: {}", state_str);

        // 端口
        if !svc.internal_ports.is_empty() {
            let ports: Vec<String> = svc.internal_ports.iter().map(|p| p.to_string()).collect();
            info!("  │  监听端口: {}", ports.join(", "));
        }

        // 网络模式
        match &svc.network_mode {
            crate::models::app::NetworkMode::Bridge => info!("  │  网络: bridge"),
            crate::models::app::NetworkMode::Host => info!("  │  网络: host"),
            crate::models::app::NetworkMode::External(name) => {
                info!("  │  网络: external ({})", name);
            }
        }

        // 路由
        if !svc.routes.is_empty() {
            info!("  │  Traefik 路由:");
            for route in &svc.routes {
                let url = format!(
                    "https://{}{}",
                    route.subdomain,
                    route.path_prefix.as_deref().unwrap_or("")
                );
                info!("  │    - {}", url);
                if route.tls {
                    info!("  │      (TLS 启用)");
                }
            }
        }

        // 端口映射
        if !svc.port_mappings.is_empty() {
            info!("  │  端口映射:");
            for pm in &svc.port_mappings {
                info!(
                    "  │    {}:{}/{:?}",
                    pm.host_port, pm.container_port, pm.protocol
                );
            }
        }

        // 数据库
        match &svc.database {
            crate::models::app::DatabaseConfig::None => {}
            crate::models::app::DatabaseConfig::SharedPostgres {
                db_name, user, ..
            } => {
                info!("  │  数据库: infra PostgreSQL");
                info!("  │    DB: {}, User: {}", db_name, user);
            }
        }

        // 卷
        if !svc.volumes.is_empty() {
            info!("  │  挂载卷:");
            for vol in &svc.volumes {
                info!(
                    "  │    {} → {} {}",
                    vol.host_path,
                    vol.container_path,
                    if vol.read_only { "(ro)" } else { "" }
                );
            }
        }

        // 共享资源挂载
        if !svc.shared_mounts.is_empty() {
            info!("  │  共享资源挂载:");
            for mount in &svc.shared_mounts {
                info!(
                    "  │    [{}] → {} {}",
                    mount.resource_name,
                    mount.container_path,
                    if mount.read_only { "(ro)" } else { "" }
                );
            }
        }

        // 环境变量
        if !svc.env_vars.is_empty() {
            info!("  │  环境变量:");
            for (key, value) in &svc.env_vars {
                // 隐藏密码类变量
                let display_value = if key.to_lowercase().contains("password")
                    || key.to_lowercase().contains("secret")
                {
                    "********"
                } else {
                    value.as_str()
                };
                info!("  │    {}={}", key, display_value);
            }
        }

        if i < app.services.len() - 1 {
            info!("  │");
        }
    }

    Ok(())
}
