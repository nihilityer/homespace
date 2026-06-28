use crate::config::Config;
use crate::models::app::{
    App, DatabaseConfig, NetworkMode, PortMapping, PortProtocol, Service,
    TraefikRoute, Volume,
};
use crate::models::compose::{ComposeService, DockerCompose};
use crate::models::middleware::Middleware;
use crate::models::status::{ContainerState, ServiceStatus};
use anyhow::Context;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::constants::*;

/// 应用概要信息（用于 list 命令）
#[derive(Debug, Clone)]
pub struct AppSummary {
    /// 应用名称
    pub name: String,
    #[allow(dead_code)]
    /// 服务数量
    pub service_count: usize,
    /// 服务名称列表
    pub service_names: Vec<String>,
    /// 路由子域名列表
    pub routes: Vec<String>,
    /// 数据库类型描述
    pub database: String,
    /// 数据路径
    pub data_path: String,
    /// 运行状态摘要
    pub status_summary: String,
}

/// 扫描 `apps_root` 下所有包含 docker-compose.yml 的子目录
pub fn scan_apps(config: &Config) -> anyhow::Result<Vec<App>> {
    let mut apps = Vec::new();
    let entries = fs::read_dir(&config.paths.apps_root)
        .with_context(|| format!("无法读取应用目录: {}", config.paths.apps_root.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let compose_file = path.join(COMPOSE_FILE);
        if !compose_file.exists() {
            continue;
        }

        if let Ok(app) = parse_app(&path, config) {
            apps.push(app);
        }
    }

    // 排序：infra 优先，其余按名称排序
    apps.sort_by(|a, b| {
        if a.name == "infra" {
            std::cmp::Ordering::Less
        } else if b.name == "infra" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    Ok(apps)
}

/// 解析单个应用目录
pub fn parse_app(dir: &std::path::Path, config: &Config) -> anyhow::Result<App> {
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let compose_file = dir.join(COMPOSE_FILE);
    let env_file = dir.join(ENV_FILE);
    let description = "".to_string(); // 可以从 README 或注释中提取

    let mut app = App {
        name: name.clone(),
        description: description.clone(),
        path: dir.to_path_buf(),
        data_root: None,
        services: Vec::new(),
    };

    // 解析 docker-compose.yml
    if compose_file.exists() {
        let content = fs::read_to_string(&compose_file)
            .with_context(|| format!("无法读取 compose 文件: {}", compose_file.display()))?;
        let compose: DockerCompose =
            serde_yaml::from_str(&content).unwrap_or_else(|_| DockerCompose {
                version: None,
                services: Default::default(),
                networks: Default::default(),
                volumes: Default::default(),
            });

        for (svc_name, svc) in &compose.services {
            let service = parse_compose_service(svc_name, svc, &name, dir, config);
            app.services.push(service);
        }
    }

    // 从 .env 读取 data_root 覆盖
    if env_file.exists() {
        let content = fs::read_to_string(&env_file)?;
        for line in content.lines() {
            if let Some(value) = parse_env_var(line, "DATA_ROOT") {
                app.data_root = Some(PathBuf::from(value));
            }
            if app.description.is_empty() {
                if let Some(desc) = parse_env_var(line, "APP_DESCRIPTION") {
                    app.description = desc;
                }
            }
        }
    }

    Ok(app)
}

/// 解析 docker-compose.yml 中的单个服务定义
fn parse_compose_service(
    name: &str,
    svc: &ComposeService,
    app_name: &str,
    app_dir: &std::path::Path,
    config: &Config,
) -> Service {
    let image = svc.image.clone().unwrap_or_default();

    // 解析端口
    let (internal_ports, port_mappings) = parse_ports(svc);

    // 解析网络模式
    let network_mode = parse_network_mode(svc);

    // 解析 Traefik 路由
    let routes = parse_traefik_routes(svc, app_name);

    // 解析中间件引用
    let middlewares = parse_middlewares(svc, app_name);

    // 解析卷
    let volumes = parse_volumes(svc, app_dir, config);

    // 解析数据库配置
    let database = infer_database_config(svc, name);

    Service {
        name: name.to_string(),
        image,
        command: svc.command.clone(),
        internal_ports,
        network_mode,
        routes,
        port_mappings,
        volumes,
        shared_mounts: Vec::new(),
        env_vars: HashMap::new(),
        database,
        middlewares,
    }
}

/// 解析端口映射（internal ports + port mappings）
fn parse_ports(svc: &ComposeService) -> (Vec<u16>, Vec<PortMapping>) {
    let mut internal = Vec::new();
    let mut mappings = Vec::new();

    if let Some(ref ports) = svc.ports {
        for port_str in ports {
            // 格式: "8080:80" 或 "8080:80/tcp"
            let parts: Vec<&str> = port_str.splitn(2, ':').collect();
            if parts.len() == 2 {
                if let Ok(host_port) = parts[0].parse::<u16>() {
                    let container_part = if let Some(stripped) = parts[1].strip_suffix("/udp") {
                        internal.push(stripped.parse().unwrap_or(0));
                        mappings.push(PortMapping {
                            host_port,
                            container_port: stripped.parse().unwrap_or(0),
                            protocol: PortProtocol::Udp,
                        });
                        continue;
                    } else if let Some(stripped) = parts[1].strip_suffix("/tcp") {
                        stripped
                    } else {
                        parts[1]
                    };

                    let container_port = container_part.parse().unwrap_or(0);
                    internal.push(container_port);
                    mappings.push(PortMapping {
                        host_port,
                        container_port,
                        protocol: PortProtocol::Tcp,
                    });
                }
            }
        }
    }

    (internal, mappings)
}

/// 解析网络模式（host/bridge/external）
fn parse_network_mode(svc: &ComposeService) -> NetworkMode {
    if let Some(ref mode) = svc.network_mode {
        match mode.as_str() {
            "host" => return NetworkMode::Host,
            "bridge" => return NetworkMode::Bridge,
            other => {
                // 检查 networks 字段中引用的外部网络
                if let Some(ref networks) = svc.networks {
                    if networks.contains_key(other) {
                        return NetworkMode::External(other.to_string());
                    }
                }
                return NetworkMode::External(other.to_string());
            }
        }
    }
    NetworkMode::Bridge
}

/// 从容器标签中解析 Traefik 路由规则
fn parse_traefik_routes(svc: &ComposeService, _app_name: &str) -> Vec<TraefikRoute> {
    let mut routes = Vec::new();
    if let Some(ref labels) = svc.labels {
        // 查找所有 traefik.http.routers.<name>.rule=Host(...) 标签
        let mut router_hosts: HashMap<String, String> = HashMap::new();
        let mut router_paths: HashMap<String, String> = HashMap::new();
        let mut router_tls: HashMap<String, bool> = HashMap::new();

        for label in labels {
            let label = label.trim();
            // traefik.http.routers.<name>.rule=Host(`xxx`)
            if let Some(rest) = label.strip_prefix("traefik.http.routers.") {
                let parts: Vec<&str> = rest.splitn(2, '=').collect();
                if parts.len() != 2 {
                    continue;
                }
                let key = parts[0].trim();
                let value = parts[1].trim();

                // key 格式: <router-name>.rule 或 <router-name>.tls 等
                let dot_pos = key.rfind('.');
                if let Some(pos) = dot_pos {
                    let router_name = &key[..pos];
                    let property = &key[pos + 1..];

                    match property {
                        "rule" => {
                            // 提取 Host(...) 部分
                            if let Some(host) = extract_host(value) {
                                router_hosts.insert(router_name.to_string(), host);
                            }
                            // 检查是否有 PathPrefix
                            if let Some(path) = extract_path_prefix(value) {
                                router_paths
                                    .insert(router_name.to_string(), path);
                            }
                        }
                        "tls" => {
                            router_tls.insert(router_name.to_string(), value == "true");
                        }
                        _ => {}
                    }
                }
            }
        }

        // 合并为路由
        for (router_name, host) in &router_hosts {
            // 跳过 Traefik 自己的仪表盘路由
            if router_name.contains("dashboard") || router_name.contains("api@internal") {
                continue;
            }
            routes.push(TraefikRoute {
                subdomain: host.clone(),
                path_prefix: router_paths.get(router_name).cloned(),
                tls: *router_tls.get(router_name).unwrap_or(&true),
            });
        }
    }

    routes
}

/// 从 Traefik 规则中提取 Host
fn extract_host(value: &str) -> Option<String> {
    // Host(`xxx.example.com`) 或 Host(`xxx`)
    let value = value.trim();
    if let Some(start) = value.find("Host(`") {
        let rest = &value[start + 6..];
        if let Some(end) = rest.find("`)") {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// 从 Traefik 规则中提取 `PathPrefix`
fn extract_path_prefix(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(start) = value.find("PathPrefix(`") {
        let rest = &value[start + 12..];
        if let Some(end) = rest.find("`)") {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// 从 docker-compose.yml 标签中解析 Traefik 文件中间件引用。
///
/// 查找 `traefik.http.routers.<name>.middlewares` 标签，
/// 从逗号分隔的值中提取 `@file` 引用并映射到 [`Middleware`] 枚举。
/// 会跳过 Traefik Dashboard 自身的路由器标签。
fn parse_middlewares(svc: &ComposeService, _app_name: &str) -> Vec<Middleware> {
    let mut result = Vec::new();
    if let Some(ref labels) = svc.labels {
        for label in labels {
            let label = label.trim();
            if !label.contains(".middlewares=") {
                continue;
            }
            let parts: Vec<&str> = label.splitn(2, '=').collect();
            if parts.len() != 2 {
                continue;
            }
            let key = parts[0].trim();
            let value = parts[1].trim();

            // 跳过 Traefik Dashboard 自身的路由器标签
            if key.contains("dashboard") || key.contains("api@internal") {
                continue;
            }

            for part in value.split(',') {
                let part = part.trim();
                if let Some(mw) = Middleware::from_name(part) {
                    if !result.contains(&mw) {
                        result.push(mw);
                    }
                }
            }
        }
    }
    result
}

/// 解析卷挂载列表
fn parse_volumes(svc: &ComposeService, _app_dir: &std::path::Path, _config: &Config) -> Vec<Volume> {
    let mut volumes = Vec::new();
    if let Some(ref svc_volumes) = svc.volumes {
        for vol_str in svc_volumes {
            let parts: Vec<&str> = vol_str.splitn(2, ':').collect();
            if parts.len() == 2 {
                let host_path = parts[0].trim().to_string();
                let container_opts: Vec<&str> = parts[1].splitn(2, ':').collect();
                let container_path = container_opts[0].trim().to_string();
                let read_only = container_opts
                    .get(1)
                    .map(|o| o.trim() == "ro")
                    .unwrap_or(false);

                // 只保留非系统路径的卷
                if host_path != "/etc/localtime"
                    && host_path != "/etc/timezone"
                    && host_path != "/var/run/docker.sock"
                    && !host_path.starts_with("./config/")
                {
                    volumes.push(Volume {
                        host_path,
                        container_path,
                        read_only,
                    });
                }
            }
        }
    }
    volumes
}

/// 推断容器使用的数据库配置。
///
/// 仅通过 `postgres` 网络检测 infra 共享 PostgreSQL 连接；
/// 应用自带数据库由镜像自行处理，不在此建模。
fn infer_database_config(svc: &ComposeService, _name: &str) -> DatabaseConfig {
    if let Some(ref networks) = svc.networks {
        if networks.contains_key(NETWORK_POSTGRES) {
            return DatabaseConfig::SharedPostgres {
                db_name: String::new(),
                user: String::new(),
                auto_create: false,
            };
        }
    }

    DatabaseConfig::None
}

/// 从 .env 文件行中解析指定键的值
fn parse_env_var(line: &str, key: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if let Some(rest) = line.strip_prefix(&format!("{}=", key)) {
        let value = rest.trim();
        // 去除引号
        let value = value.trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            return None;
        }
        return Some(value.to_string());
    }
    None
}

/// 获取应用运行状态
pub fn get_app_status(app: &App) -> anyhow::Result<Vec<ServiceStatus>> {
    let mut statuses = Vec::new();
    let output = std::process::Command::new("docker")
        .args(["compose", "-f"])
        .arg(app.path.join(COMPOSE_FILE))
        .args(["ps", "--format", "json"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // 解析 JSON 行
            for line in stdout.lines() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                    let svc_name = parsed["Service"]
                        .as_str()
                        .unwrap_or("?")
                        .to_string();
                    let state = match parsed["State"].as_str().unwrap_or("") {
                        "running" => ContainerState::Running,
                        "exited" | "stopped" => ContainerState::Stopped,
                        "paused" => ContainerState::Paused,
                        "" => ContainerState::NotFound,
                        _ => ContainerState::Unknown,
                    };

                    // 查找对应 service 的路由
                    let routes: Vec<String> = app
                        .services
                        .iter()
                        .filter(|s| s.name == svc_name)
                        .flat_map(|s| s.routes.iter().map(|r| r.subdomain.clone()))
                        .collect();

                    statuses.push(ServiceStatus {
                        name: svc_name,
                        state,
                        routes,
                    });
                }
            }
        }
        _ => {
            // docker compose ps 失败，所有服务标记为 NotDeployed
            for svc in &app.services {
                statuses.push(ServiceStatus {
                    name: svc.name.clone(),
                    state: ContainerState::NotFound,
                    routes: svc.routes.iter().map(|r| r.subdomain.clone()).collect(),
                });
            }
        }
    }

    Ok(statuses)
}

/// 生成 list 命令的摘要
pub fn build_summary(app: &App, statuses: &[ServiceStatus]) -> AppSummary {
    let routes: Vec<String> = app
        .services
        .iter()
        .flat_map(|s| s.routes.iter().map(|r| r.subdomain.clone()))
        .collect();

    let database = app
        .services
        .iter()
        .find_map(|s| match &s.database {
            DatabaseConfig::SharedPostgres { .. } => Some("infra PG".to_string()),
            DatabaseConfig::None => None,
        })
        .unwrap_or_else(|| "-".to_string());

    let running_count = statuses
        .iter()
        .filter(|s| matches!(s.state, ContainerState::Running))
        .count();
    let total_count = statuses.len();
    let status_summary = if running_count == total_count && total_count > 0 {
        "running".to_string()
    } else if running_count == 0 {
        "stopped".to_string()
    } else if running_count > 0 {
        "partial".to_string()
    } else {
        "not deployed".to_string()
    };

    let data_path = app
        .data_root
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "-".to_string());

    AppSummary {
        name: app.name.clone(),
        service_count: app.services.len(),
        service_names: app.services.iter().map(|s| s.name.clone()).collect(),
        routes,
        database,
        data_path,
        status_summary,
    }
}

/// 检查端口冲突
pub fn check_port_conflict(port: u16, apps: &[App]) -> Option<String> {
    for app in apps {
        for svc in &app.services {
            for mapping in &svc.port_mappings {
                if mapping.host_port == port {
                    return Some(format!(
                        "端口 {} 已被 {} 的 service '{}' 占用",
                        port, app.name, svc.name
                    ));
                }
            }
        }
    }
    None
}

/// 检查子域名冲突
pub fn check_domain_conflict(domain: &str, apps: &[App]) -> Option<String> {
    for app in apps {
        for svc in &app.services {
            for route in &svc.routes {
                if route.subdomain == domain {
                    return Some(format!(
                        "域名 {} 已被 {} 的 service '{}' 注册",
                        domain, app.name, svc.name
                    ));
                }
            }
        }
    }
    None
}
