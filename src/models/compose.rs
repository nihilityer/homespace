use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// docker-compose.yml 顶层结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerCompose {
    /// Compose 版本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    /// 服务列表
    pub services: BTreeMap<String, ComposeService>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    /// 网络列表
    pub networks: BTreeMap<String, ComposeNetwork>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    /// 数据卷列表
    pub volumes: BTreeMap<String, ComposeVolume>,
}

/// Docker Compose 服务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeService {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Docker 镜像
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 容器名
    pub container_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 启动命令
    pub command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 重启策略
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 网络模式
    pub network_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// 附加网络
    pub networks: Option<BTreeMap<String, ComposeServiceNetwork>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 端口映射
    pub ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 数据卷
    pub volumes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 环境变量
    pub environment: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 环境变量文件
    pub env_file: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 标签
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 服务依赖
    pub depends_on: Option<BTreeMap<String, DependsOnCondition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 健康检查
    pub healthcheck: Option<ComposeHealthCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 运行用户
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 日志配置
    pub logging: Option<ComposeLogging>,
}

/// 服务网络别名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceNetwork {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 网络别名
    pub aliases: Option<Vec<String>>,
}

/// 依赖条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependsOnCondition {
    /// 依赖条件（如 `service_started`）
    pub condition: String,
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHealthCheck {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 检查命令
    pub test: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 检查间隔
    pub interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 超时时间
    pub timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 重试次数
    pub retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 启动等待时间
    pub start_period: Option<String>,
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeLogging {
    /// 日志驱动
    pub driver: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 日志选项
    pub options: Option<BTreeMap<String, String>>,
}

impl ComposeLogging {
    /// 创建一个标准的 `json-file` 日志配置。
    ///
    /// `max_size` 指定单个日志文件的最大大小（如 `"10m"`、`"1m"`）。
    #[must_use]
    pub fn json_file(max_size: &str) -> Self {
        let mut opts = BTreeMap::new();
        opts.insert("max-size".to_string(), max_size.to_string());
        Self {
            driver: "json-file".to_string(),
            options: Some(opts),
        }
    }
}

/// Compose 网络
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeNetwork {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 是否为外部网络
    pub external: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 网络名
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 网络驱动
    pub driver: Option<String>,
}

/// Compose 数据卷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeVolume {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 是否为外部数据卷
    pub external: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 数据卷名
    pub name: Option<String>,
}
