use tracing::info;
use crate::config::Config;
use crate::services::docker;
use crate::services::scanner;

/// 对指定应用执行生命周期操作。
///
/// `action` 可以是 `"start"`、`"stop"`、`"restart"`、`"pull"`、`"build"` 之一。
/// 执行失败时返回错误。
pub fn run(
    app_name: &str,
    config: &Config,
    action: &str,
    _commit: bool,
    _no_git: bool,
) -> anyhow::Result<()> {
    // 1. 扫描已有应用
    let apps = scanner::scan_apps(config)?;

    // 2. 按名称查找
    let app = apps
        .iter()
        .find(|a| a.name == app_name)
        .ok_or_else(|| anyhow::anyhow!("未找到应用: {}", app_name))?;

    // 3. 根据 action 分发到对应的 docker 操作
    match action {
        "start" => {
            info!("🚀 正在启动 {}...", app_name);
            docker::compose_up(&app.path)?;
            info!("  ✅ {} 启动完成", app_name);
        }
        "stop" => {
            info!("⏹️  正在停止 {}...", app_name);
            docker::compose_stop(&app.path)?;
            info!("  ✅ {} 停止完成", app_name);
        }
        "restart" => {
            info!("🔄 正在重启 {}...", app_name);
            docker::compose_restart(&app.path)?;
            info!("  ✅ {} 重启完成", app_name);
        }
        "pull" => {
            info!("⬇️  正在拉取 {} 的最新镜像...", app_name);
            docker::compose_pull(&app.path)?;
            info!("  ✅ {} 镜像拉取完成", app_name);
        }
        "build" => {
            info!("🔨 正在构建 {}...", app_name);
            docker::compose_build(&app.path)?;
            info!("  ✅ {} 构建完成", app_name);
        }
        other => {
            anyhow::bail!("未知的生命周期操作: {}", other);
        }
    }

    Ok(())
}
