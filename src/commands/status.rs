use tracing::info;
use crate::config::Config;
use crate::services::database;
use crate::services::docker;
use crate::constants::*;

/// 检查基础设施（Traefik、PostgreSQL、Docker 网络）状态
#[allow(clippy::cognitive_complexity)]
pub fn run(config: &Config) -> anyhow::Result<()> {
    info!("🔍 homespace - 基础设施状态\n");

    // Traefik 状态
    match docker::container_exists(CONTAINER_TRAEFIK) {
        Ok(true) => info!("Traefik 容器状态: ✅ 存在"),
        Ok(false) => info!("Traefik 容器状态: ❌ 未找到"),
        Err(e) => info!("Traefik 容器状态: ⚠️  检查失败: {}", e),
    }

    match docker::traefik_ping() {
        Ok(true) => info!("Traefik /ping 端点: ✅ 响应正常"),
        Ok(false) => info!("Traefik /ping 端点: ⚠️  无响应"),
        Err(e) => info!("Traefik /ping 端点: ⚠️  检查失败: {}", e),
    }

    // Docker 网络状态
    info!("");
    for network in &[NETWORK_TRAEFIK, NETWORK_POSTGRES] {
        match docker::network_exists(network) {
            Ok(true) => info!("Docker 网络 {}: ✅ 存在", network),
            Ok(false) => info!("Docker 网络 {}: ❌ 不存在", network),
            Err(e) => info!("Docker 网络 {}: ⚠️  检查失败: {}", network, e),
        }
    }

    // PostgreSQL 状态
    match database::test_connection(NETWORK_POSTGRES) {
        Ok(true) => info!("PostgreSQL 连接测试: ✅ 连接成功"),
        Ok(false) => info!("PostgreSQL 连接测试: ⚠️  无法连接（网络或容器可能未启动）"),
        Err(e) => info!("PostgreSQL 连接测试: ⚠️  测试失败: {}", e),
    }

    // SSL 证书检查
    info!("\nSSL 证书: ");
    let ssl_path = config.paths.apps_root.join(TRAEFIK_DIR).join(SSL_DIR);
    if ssl_path.exists() {
        info!("  证书目录: {} ✅", ssl_path.display());
        // 检查 acme.json
        let acme_path = ssl_path.join("acme.json");
        if acme_path.exists()
            && let Ok(meta) = std::fs::metadata(&acme_path) {
                info!("  acme.json: {} bytes", meta.len());
                info!("  ⚠️  证书到期时间需手动检查 (acme.json 为 Traefik 内部格式)");
            }
    } else {
        info!("  ⚠️  证书目录未找到: {}", ssl_path.display());
    }

    // 检查 Traefik compose 文件
    let traefik_compose = config.traefik_compose_path();
    info!("\nTraefik docker-compose.yml: {}", traefik_compose.display());
    if traefik_compose.exists() {
        info!("  ✅ 文件存在");

        // 尝试获取 infra 容器状态
        if let Ok(output) = std::process::Command::new("docker")
            .args(["compose", "-f"])
            .arg(&traefik_compose)
            .args(["ps", "--format", "table {{.Name}}\t{{.Status}}"])
            .output()
            && output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.trim().is_empty() {
                    info!("\n  容器状态:");
                    for line in stdout.lines() {
                        info!("    {}", line);
                    }
                }
            }
    } else {
        info!("  ❌ 文件不存在，infra 可能未部署");
    }

    Ok(())
}
