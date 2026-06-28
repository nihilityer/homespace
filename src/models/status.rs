//! 应用运行时状态类型 — 描述容器的运行状态，与持久化的域模型分离。
//!
//! 这些类型由 [`scanner`](crate::services::scanner) 通过 Docker CLI 查询填充，
//! 不参与配置文件的序列化与反序列化。

/// 运行中的应用状态。
///
/// 由 `scanner::get_app_status()` 返回，包含应用中所有服务的容器状态。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AppStatus {
    /// 应用名
    pub app_name: String,
    /// 服务状态列表
    pub services: Vec<ServiceStatus>,
}

/// 单个服务的运行状态。
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    /// 服务名
    pub name: String,
    /// 容器状态
    pub state: ContainerState,
    /// 路由列表（子域名）
    #[allow(dead_code)]
    pub routes: Vec<String>,
}

/// 容器运行状态枚举。
#[derive(Debug, Clone)]
pub enum ContainerState {
    /// 运行中
    Running,
    /// 已停止
    Stopped,
    /// 已暂停
    Paused,
    /// 未找到（容器不存在或未部署）
    NotFound,
    /// 未知状态（Docker 返回了无法识别的状态字符串）
    Unknown,
}
