use tracing::info;
use crate::config::Config;
use crate::services::scanner;

/// 列出所有已扫描应用及其状态
pub fn run(config: &Config) -> anyhow::Result<()> {
    let apps = scanner::scan_apps(config)?;

    if apps.is_empty() {
        info!("未发现任何应用。请先运行 homespace init 或确保 apps_root 下有 docker-compose.yml 文件。");
        return Ok(());
    }

    info!(
        "  {:<14} {:<12} {:<10} {:<32} {:<14} {:<}",
        "App", "Services", "Status", "Routes", "DB", "Data Path"
    );
    info!(
        "  {:-<14} {:-<12} {:-<10} {:-<32} {:-<14} {:-<}",
        "", "", "", "", "", ""
    );

    for app in &apps {
        let statuses = scanner::get_app_status(app).unwrap_or_default();
        let summary = scanner::build_summary(app, &statuses);

        let status_icon = match summary.status_summary.as_str() {
            "running" => "🟢",
            "stopped" => "🔴",
            "partial" => "🟡",
            _ => "⚪",
        };

        // 第一行：应用名 + 第一个 service
        let first_svc = summary.service_names.first().cloned().unwrap_or_default();
        let routes_str = if summary.routes.is_empty() {
            "-".to_string()
        } else {
            summary.routes.first().cloned().unwrap_or_default()
        };

        info!(
            "  {:<14} {:<12} {}{:<9} {:<32} {:<14} {:<}",
            summary.name,
            first_svc,
            status_icon,
            format!(" {}", summary.status_summary),
            routes_str,
            summary.database,
            summary.data_path
        );

        // 后续 service 行（缩进）
        for svc_name in summary.service_names.iter().skip(1) {
            let route = app
                .services
                .iter()
                .find(|s| &s.name == svc_name)
                .and_then(|s| s.routes.first())
                .map(|r| r.subdomain.clone())
                .unwrap_or_else(|| "-".to_string());

            info!(
                "  {:<14} {:<12} {:<10} {:<32} {:<14} {:<}",
                "", svc_name, "", route, "", ""
            );
        }
    }

    Ok(())
}
