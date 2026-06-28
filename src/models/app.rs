use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::models::middleware::Middleware;

/// 一个 `HomeLab` 应用 = 一个 docker compose 项目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    /// 应用名
    pub name: String,
    /// 应用描述
    pub description: String,
    /// 应用本地路径
    pub path: PathBuf,
    /// 覆盖全局 `data_root，None` 表示使用全局默认
    pub data_root: Option<PathBuf>,
    /// 服务列表
    pub services: Vec<Service>,
}

/// Docker Compose 中的一个 service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    /// 服务名
    pub name: String,
    /// Docker 镜像
    pub image: String,
    /// 容器启动命令，覆盖镜像默认 CMD。`None` 表示使用镜像默认命令
    pub command: Option<Vec<String>>,
    /// 内部端口列表
    pub internal_ports: Vec<u16>,
    /// 网络模式
    pub network_mode: NetworkMode,
    /// Traefik 路由
    pub routes: Vec<TraefikRoute>,
    /// 端口映射
    pub port_mappings: Vec<PortMapping>,
    /// 数据卷挂载
    pub volumes: Vec<Volume>,
    /// 共享资源挂载
    pub shared_mounts: Vec<SharedResourceMount>,
    /// 环境变量
    pub env_vars: HashMap<String, String>,
    /// 数据库配置
    pub database: DatabaseConfig,
    /// Traefik 文件中间件列表（引用 `@file` 提供者）
    #[serde(default)]
    pub middlewares: Vec<Middleware>,
}

/// 网络模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMode {
    /// 桥接模式
    Bridge,
    /// 主机模式
    Host,
    /// 指定外部 Docker 网络名
    External(String),
}

/// Traefik 路由配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraefikRoute {
    /// 子域名
    pub subdomain: String,
    /// URL 路径前缀
    pub path_prefix: Option<String>,
}

/// 端口映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    /// 宿主机端口
    pub host_port: u16,
    /// 容器端口
    pub container_port: u16,
    /// 协议
    pub protocol: PortProtocol,
}

/// 端口协议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PortProtocol {
    /// TCP
    Tcp,
    /// UDP
    Udp,
}

/// 数据库配置。
///
/// 仅区分是否需要 infra 共享 PostgreSQL；应用自带数据库由镜像自行处理，
/// 无需在此建模。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseConfig {
    /// 无需数据库
    None,
    /// 使用 infra 共享 `PostgreSQL`
    SharedPostgres {
        /// 数据库名
        db_name: String,
        /// 用户名
        user: String,
        /// 是否在 infra 上自动创建数据库和用户
        auto_create: bool,
    },
}

/// 数据卷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    /// 宿主机路径
    pub host_path: String,
    /// 容器路径
    pub container_path: String,
    /// 是否只读
    pub read_only: bool,
}

/// 全局定义的共享资源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedResource {
    /// 资源名
    pub name: String,
    /// 资源路径
    pub path: PathBuf,
    /// 资源描述
    pub description: String,
    /// 是否只读
    pub read_only: bool,
    /// 文件 UID
    pub uid: Option<u32>,
    /// 文件 GID
    pub gid: Option<u32>,
}

/// service 对共享资源的挂载引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedResourceMount {
    /// 资源名
    pub resource_name: String,
    /// 容器内路径
    pub container_path: String,
    /// 是否只读
    pub read_only: bool,
}
