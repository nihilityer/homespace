//! 全局常量 — 网络名、容器名、目录名等共享字符串。

// ── Docker 网络 ──
/// Traefik 边缘路由网络（外部可见）
pub const NETWORK_TRAEFIK: &str = "traefik";
/// `PostgreSQL` 数据库网络（bridge）
pub const NETWORK_POSTGRES: &str = "postgres";

// ── 容器名 ──
/// Traefik 容器名
pub const CONTAINER_TRAEFIK: &str = "traefik";
/// `PostgreSQL` 容器名
pub const CONTAINER_POSTGRES: &str = "postgres";

// ── 目录 ──
/// infra 基础设施目录
pub const INFRA_DIR: &str = "infra";
/// Traefik 配置目录
pub const TRAEFIK_DIR: &str = "traefik";
/// `PostgreSQL` 数据目录
pub const POSTGRES_DIR: &str = "postgres";
/// 配置目录名
pub const CONFIG_DIR: &str = "config";
/// SSL 证书目录
pub const SSL_DIR: &str = "ssl";
/// 数据目录名
pub const DATA_DIR: &str = "data";

// ── 文件名 ──
/// Docker Compose 文件名
pub const COMPOSE_FILE: &str = "docker-compose.yml";
/// 环境变量文件名
pub const ENV_FILE: &str = ".env";
/// 环境变量示例文件名
pub const ENV_EXAMPLE_FILE: &str = ".env.example";
/// Git 忽略文件名
pub const GITIGNORE_FILE: &str = ".gitignore";
/// 说明文件名
pub const README_FILE: &str = "README.md";
/// 用户列表文件名
pub const USERS_FILE: &str = "users.txt";
/// TLS 配置文件名
pub const TLS_CONFIG_FILE: &str = "tls.toml";

// ── 静态站点 ──
/// Nginx 静态站点目录
pub const NGINX_SITE_DIR: &str = "site";
/// Nginx 配置文件名
pub const NGINX_CONF_FILE: &str = "nginx.conf";
/// 静态站点默认 nginx 镜像
pub const NGINX_IMAGE: &str = "nginx:alpine";

// ── homespace 配置 ──
/// homespace 配置目录
pub const HOMESPACE_CONFIG_DIR: &str = ".config/homespace";
/// homespace 配置文件名
pub const HOMESPACE_CONFIG_FILE: &str = "config.toml";
