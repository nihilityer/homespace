//! 基础设施生成 — 从零创建基础设施项目（`Traefik` + `PostgreSQL`），
//! 作为两个独立的 Docker Compose 项目以便独立管理。
//!
//! 中间件采用文件化配置，每个中间件一个文件，存放于 `config/` 目录。

use anyhow::Context;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::compose::*;
use crate::models::middleware::Middleware;
use crate::constants::*;

/// 基础设施生成参数
pub struct InfraConfig {
    /// 主域名（用于 DNS 和 ACME）
    pub domain: String,
    /// ACME 注册邮箱
    pub acme_email: String,
    /// Cloudflare API Token
    pub cf_api_token: String,
    /// Traefik 面板访问域名
    pub service_domain: String,
    /// HTTP 端口
    pub http_port: u16,
    /// HTTPS 端口
    pub https_port: u16,
    /// Traefik 镜像版本
    pub traefik_version: String,
    /// `PostgreSQL` 镜像版本
    pub postgres_version: String,
    /// `PostgreSQL` 用户名
    pub pg_user: String,
    /// `PostgreSQL` 密码
    pub pg_password: String,
    /// `PostgreSQL` 默认数据库名
    pub pg_db: String,
    /// 基础设施项目根路径
    pub infra_path: PathBuf,
    /// 时区
    pub timezone: String,
    /// Dashboard 用户凭据：`(用户名, htpasswd 格式条目)`
    pub dashboard_user: Option<(String, String)>,
}

/// 在 `infra_path` 下生成 `Traefik` 和 `PostgreSQL` 项目
pub fn generate_infra(cfg: &InfraConfig) -> anyhow::Result<()> {
    let traefik_dir = cfg.infra_path.join(TRAEFIK_DIR);
    let postgres_dir = cfg.infra_path.join(POSTGRES_DIR);

    generate_traefik(cfg, &traefik_dir)?;
    generate_postgres(cfg, &postgres_dir)?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Traefik
// ═══════════════════════════════════════════════════════════════════════

/// 生成 Traefik 项目目录和文件
fn generate_traefik(cfg: &InfraConfig, dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dir.join(CONFIG_DIR))
        .with_context(|| format!("无法创建目录: {}", dir.join(CONFIG_DIR).display()))?;
    fs::create_dir_all(dir.join(SSL_DIR))
        .with_context(|| format!("无法创建目录: {}", dir.join(SSL_DIR).display()))?;

    // .gitignore
    fs::write(dir.join(GITIGNORE_FILE), ".env\nssl/\n*.log\n")?;

    // config/tls.toml — TLS 加密套件
    fs::write(dir.join(CONFIG_DIR).join(TLS_CONFIG_FILE), TLS_TOML)?;

    // config/*.yml — Traefik 文件化中间件
    for mw in Middleware::all() {
        let mw_path = dir.join(CONFIG_DIR).join(mw.filename());
        fs::write(&mw_path, mw.yaml_content())
            .with_context(|| format!("无法写入中间件文件: {}", mw_path.display()))?;
    }

    // config/users.txt — 基础认证用户文件
    let users_content = if let Some((ref _user, ref entry)) = cfg.dashboard_user {
        format!(
            "# Dashboard 认证用户（由 homespace 生成）\n\
             # 添加更多用户: htpasswd -nB <username> >> config/users.txt\n\
             {entry}\n",
        )
    } else {
        "# Basic Auth 用户文件（占位）\n\
         # 添加用户: htpasswd -nB <username> >> config/users.txt\n\
         # 示例: admin:$2y$10$...\n"
            .to_string()
    };
    fs::write(dir.join(CONFIG_DIR).join(USERS_FILE), users_content)?;

    // .env
    let env = build_traefik_env(cfg)?;
    fs::write(dir.join(ENV_FILE), &env)?;

    // docker-compose.yml
    let compose = build_traefik_compose(cfg)?;
    let yaml =
        serde_yaml::to_string(&compose).context("序列化 traefik docker-compose.yml 失败")?;
    let header = "# Traefik — 边缘路由 / HTTPS\n# 由 homespace 生成\n\n";
    fs::write(dir.join(COMPOSE_FILE), format!("{}{}", header, yaml))?;

    Ok(())
}

/// 构建 Traefik 的 .env 文件内容
fn build_traefik_env(cfg: &InfraConfig) -> anyhow::Result<String> {
    let mut buf = String::new();
    buf.push_str("# Traefik 版本\n");
    buf.push_str(&format!("TRAEFIK_VERSION={}\n", cfg.traefik_version));
    buf.push('\n');
    buf.push_str("# 服务端口\n");
    buf.push_str(&format!("SERVICE_HTTP_PORT={}\n", cfg.http_port));
    buf.push_str(&format!("SERVICE_HTTPS_PORT={}\n", cfg.https_port));
    buf.push_str(&format!("SERVICE_DOMAIN={}\n", cfg.service_domain));
    buf.push('\n');
    buf.push_str("# DNS / ACME\n");
    buf.push_str(&format!("DNS_MAIN={}\n", cfg.domain));
    buf.push_str(&format!("DNS_LIST=*.{}\n", cfg.domain));
    buf.push_str(&format!("ACME_EMAIL={}\n", cfg.acme_email));
    buf.push_str("ACME_PROVIDER=cloudflare\n");
    buf.push_str(&format!("CF_DNS_API_TOKEN={}\n", cfg.cf_api_token));
    buf.push('\n');
    buf.push_str(&format!("TZ={}\n", cfg.timezone));
    Ok(buf)
}

/// 构建 Traefik 的 `DockerCompose` 结构
fn build_traefik_compose(_icfg: &InfraConfig) -> anyhow::Result<DockerCompose> {
    let mut services: BTreeMap<String, ComposeService> = BTreeMap::new();
    let mut networks: BTreeMap<String, ComposeNetwork> = BTreeMap::new();

    // Traefik 网络
    let mut traefik_networks = BTreeMap::new();
    traefik_networks.insert(NETWORK_TRAEFIK.into(), ComposeServiceNetwork { aliases: None });

    services.insert(
        CONTAINER_TRAEFIK.into(),
        ComposeService {
            image: Some("traefik:${TRAEFIK_VERSION}".into()),
            container_name: Some(CONTAINER_TRAEFIK.into()),
            restart: Some("always".into()),
            command: Some(traefik_command()),
            network_mode: None,
            networks: Some(traefik_networks),
            ports: Some(vec![
                "${SERVICE_HTTP_PORT}:${SERVICE_HTTP_PORT}".into(),
                "${SERVICE_HTTPS_PORT}:${SERVICE_HTTPS_PORT}".into(),
                "${SERVICE_HTTPS_PORT}:${SERVICE_HTTPS_PORT}/udp".into(),
            ]),
            volumes: Some(vec![
                "/etc/localtime:/etc/localtime:ro".into(),
                "/etc/timezone:/etc/timezone:ro".into(),
                "/var/run/docker.sock:/var/run/docker.sock:ro".into(),
                "./config/:/etc/traefik/config/:ro".into(),
                "./ssl/:/data/ssl/".into(),
            ]),
            environment: None,
            env_file: Some(vec![ENV_FILE.into()]),
            labels: Some(traefik_labels()),
            depends_on: None,
            healthcheck: Some(ComposeHealthCheck {
                test: Some(vec![
                    "CMD-SHELL".into(),
                    "wget -q --spider --proxy off localhost:8080/ping || exit 1".into(),
                ]),
                interval: Some("3s".into()),
                timeout: None,
                retries: Some(10),
                start_period: None,
            }),
            user: None,
            logging: Some(ComposeLogging::json_file("1m")),
        },
    );

    networks.insert(
        NETWORK_TRAEFIK.into(),
        ComposeNetwork {
            external: None,
            name: Some(NETWORK_TRAEFIK.into()),
            driver: Some("bridge".into()),
        },
    );

    Ok(DockerCompose {
        version: Some("3.8".into()),
        services,
        networks,
        volumes: BTreeMap::new(),
    })
}

/// Traefik 启动命令（静态配置）
fn traefik_command() -> Vec<String> {
    vec![
        // 全局设置
        "--global.sendanonymoususage=false".into(),
        "--global.checknewversion=false".into(),
        // 入口点
        "--entrypoints.http.address=:${SERVICE_HTTP_PORT}".into(),
        "--entrypoints.https.address=:${SERVICE_HTTPS_PORT}".into(),
        // HTTPS 设为默认入口
        "--entryPoints.https.asDefault=true".into(),
        // HTTP/3 支持
        "--entryPoints.https.http3".into(),
        "--entryPoints.https.http3.advertisedport=${SERVICE_HTTPS_PORT}".into(),
        "--serverstransport.insecureskipverify=true".into(),
        // 转发头信任
        "--entryPoints.http.forwardedHeaders.trustedIPs=127.0.0.1/32,172.18.0.1/24".into(),
        "--entryPoints.https.forwardedHeaders.trustedIPs=127.0.0.1/32,172.18.0.1/24".into(),
        // API / Dashboard / Ping
        "--api=true".into(),
        "--api.dashboard=true".into(),
        "--ping=true".into(),
        // 日志
        "--log.level=INFO".into(),
        "--log.maxsize=100".into(),
        "--log.format=common".into(),
        "--accesslog=false".into(),
        // Docker 提供者
        "--providers.docker=true".into(),
        "--providers.docker.watch=true".into(),
        "--providers.docker.exposedbydefault=false".into(),
        "--providers.docker.endpoint=unix:///var/run/docker.sock".into(),
        "--providers.docker.useBindPortIP=false".into(),
        format!("--providers.docker.network={NETWORK_TRAEFIK}"),
        // 文件提供者（中间件 + TLS）
        "--providers.file=true".into(),
        "--providers.file.watch=true".into(),
        "--providers.file.directory=/etc/traefik/config".into(),
        "--providers.file.debugloggeneratedtemplate=true".into(),
        // ACME 自动证书（Cloudflare DNS 验证）
        "--certificatesresolvers.cloudflare.acme.email=${ACME_EMAIL}".into(),
        "--certificatesresolvers.cloudflare.acme.storage=/data/ssl/acme.json".into(),
        "--certificatesresolvers.cloudflare.acme.dnsChallenge.resolvers=1.1.1.1:53,8.8.8.8:53".into(),
        "--certificatesresolvers.cloudflare.acme.dnsChallenge.provider=${ACME_PROVIDER}".into(),
        "--certificatesresolvers.cloudflare.acme.dnsChallenge.propagation.delayBeforeChecks=30".into(),
    ]
}

/// Traefik 容器标签（路由器定义，引用文件中间件）
fn traefik_labels() -> Vec<String> {
    // 构建中间件链字符串
    let dashboard_mw = [
        Middleware::Gzip,
        Middleware::XForwardedProto,
        Middleware::IpAllowlistInternal,
        Middleware::DashboardAuth,
    ];
    let dashboard_mw_chain = dashboard_mw.iter()
        .map(|m| m.label_ref())
        .collect::<Vec<_>>()
        .join(",");

    let api_mw = [Middleware::Gzip, Middleware::IpAllowlistInternal];
    let api_mw_chain = api_mw.iter()
        .map(|m| m.label_ref())
        .collect::<Vec<_>>()
        .join(",");

    vec![
        // 服务发现
        "traefik.enable=true".into(),
        format!("traefik.docker.network={NETWORK_TRAEFIK}"),
        // ── 仪表盘路由器（HTTPS，内网 + 基础认证）──
        "traefik.http.routers.traefik-dashboard-secure.tls.certresolver=cloudflare".into(),
        "traefik.http.routers.traefik-dashboard-secure.tls.domains[0].main=${DNS_MAIN}".into(),
        "traefik.http.routers.traefik-dashboard-secure.tls.domains[0].sans=${DNS_LIST}".into(),
        "traefik.http.routers.traefik-dashboard-secure.tls=true".into(),
        "traefik.http.routers.traefik-dashboard-secure.entrypoints=https".into(),
        format!("traefik.http.routers.traefik-dashboard-secure.middlewares={dashboard_mw_chain}"),
        "traefik.http.routers.traefik-dashboard-secure.rule=Host(`${SERVICE_DOMAIN}`)".into(),
        "traefik.http.routers.traefik-dashboard-secure.service=dashboard@internal".into(),
        // ── HTTP → HTTPS 重定向 ──
        "traefik.http.routers.traefik-dashboard-nosecure.entrypoints=http".into(),
        format!(
            "traefik.http.routers.traefik-dashboard-nosecure.middlewares={}",
            Middleware::RedirectHttps.label_ref()
        ),
        "traefik.http.routers.traefik-dashboard-nosecure.rule=Host(`${SERVICE_DOMAIN}`)".into(),
        "traefik.http.routers.traefik-dashboard-nosecure.service=noop@internal".into(),
        // ── API 路由器（HTTPS，内网白名单）──
        "traefik.http.routers.traefik-dashboard-api.tls=true".into(),
        "traefik.http.routers.traefik-dashboard-api.entrypoints=https".into(),
        format!("traefik.http.routers.traefik-dashboard-api.middlewares={api_mw_chain}"),
        "traefik.http.routers.traefik-dashboard-api.rule=Host(`${SERVICE_DOMAIN}`) && (PathPrefix(`/api`) || PathPrefix(`/dashboard`))".into(),
        "traefik.http.routers.traefik-dashboard-api.service=api@internal".into(),
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// PostgreSQL
// ═══════════════════════════════════════════════════════════════════════

/// 生成 `PostgreSQL` 项目目录和文件
fn generate_postgres(cfg: &InfraConfig, dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dir.join(DATA_DIR))
        .with_context(|| format!("无法创建目录: {}", dir.join(DATA_DIR).display()))?;

    // .gitignore
    fs::write(dir.join(GITIGNORE_FILE), ".env\ndata/\n")?;

    // .env
    let env = build_postgres_env(cfg)?;
    fs::write(dir.join(ENV_FILE), &env)?;

    // docker-compose.yml
    let compose = build_postgres_compose()?;
    let yaml =
        serde_yaml::to_string(&compose).context("序列化 postgres docker-compose.yml 失败")?;
    let header = "# PostgreSQL — 共享数据库\n# 由 homespace 生成\n\n";
    fs::write(dir.join(COMPOSE_FILE), format!("{}{}", header, yaml))?;

    Ok(())
}

/// 构建 `PostgreSQL` 的 .env 文件内容
fn build_postgres_env(cfg: &InfraConfig) -> anyhow::Result<String> {
    let mut buf = String::new();
    buf.push_str("# PostgreSQL 版本\n");
    buf.push_str(&format!("POSTGRES_VERSION={}\n", cfg.postgres_version));
    buf.push_str("# 认证\n");
    buf.push_str(&format!("POSTGRES_USER={}\n", cfg.pg_user));
    buf.push_str(&format!("POSTGRES_PASSWORD={}\n", cfg.pg_password));
    buf.push_str(&format!("POSTGRES_DB={}\n", cfg.pg_db));
    buf.push_str("# 数据目录\n");
    buf.push_str("DATA_ROOT=./data\n");
    buf.push('\n');
    buf.push_str(&format!("TZ={}\n", cfg.timezone));
    Ok(buf)
}

/// 构建 `PostgreSQL` 的 `DockerCompose` 结构
fn build_postgres_compose() -> anyhow::Result<DockerCompose> {
    let mut services: BTreeMap<String, ComposeService> = BTreeMap::new();
    let mut networks: BTreeMap<String, ComposeNetwork> = BTreeMap::new();

    let mut pg_networks = BTreeMap::new();
    pg_networks.insert(NETWORK_POSTGRES.into(), ComposeServiceNetwork { aliases: None });

    services.insert(
        CONTAINER_POSTGRES.into(),
        ComposeService {
            image: Some("postgres:${POSTGRES_VERSION}".into()),
            container_name: Some(CONTAINER_POSTGRES.into()),
            restart: Some("unless-stopped".into()),
            command: None,
            network_mode: None,
            networks: Some(pg_networks),
            ports: Some(vec!["5433:5432".into()]),
            volumes: Some(vec![
                "${DATA_ROOT}/postgres:/var/lib/postgresql/data".into(),
            ]),
            environment: Some(vec![
                "POSTGRES_USER=${POSTGRES_USER}".into(),
                "POSTGRES_PASSWORD=${POSTGRES_PASSWORD}".into(),
                "POSTGRES_DB=${POSTGRES_DB}".into(),
                "PGDATA=/var/lib/postgresql/data/pgdata".into(),
            ]),
            env_file: None,
            labels: None,
            depends_on: None,
            healthcheck: Some(ComposeHealthCheck {
                test: Some(vec![
                    "CMD-SHELL".into(),
                    "pg_isready -U ${POSTGRES_USER} -d ${POSTGRES_DB}".into(),
                ]),
                interval: Some("10s".into()),
                timeout: Some("5s".into()),
                retries: Some(5),
                start_period: None,
            }),
            user: None,
            logging: Some(ComposeLogging::json_file("10m")),
        },
    );

    networks.insert(
        NETWORK_POSTGRES.into(),
        ComposeNetwork {
            external: None,
            name: Some(NETWORK_POSTGRES.into()),
            driver: Some("bridge".into()),
        },
    );

    Ok(DockerCompose {
        version: Some("3.8".into()),
        services,
        networks,
        volumes: BTreeMap::new(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// 静态配置文件内容
// ═══════════════════════════════════════════════════════════════════════

/// TLS 加密套件配置
const TLS_TOML: &str = r#"[tls.options.default]
minVersion = "VersionTLS12"
sniStrict = false
cipherSuites = [
  "TLS_AES_128_GCM_SHA256",
  "TLS_AES_256_GCM_SHA384",
  "TLS_CHACHA20_POLY1305_SHA256",
  "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
  "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
  "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
]
"#;
