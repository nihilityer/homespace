/// 交互式添加新应用
pub mod add;
/// 交互式添加静态网站应用
pub mod add_static;
/// 修改已有应用配置
pub mod edit;
/// 初始化 homespace 环境
pub mod init;
/// 应用生命周期管理（启动、停止、重启、拉取、构建）
pub mod lifecycle;
/// 列出所有应用及其状态
pub mod list;
/// 移除应用
pub mod remove;
/// 管理共享资源目录
pub mod resource;
/// 升级应用镜像版本
pub mod upgrade;
/// 查看应用详细配置
pub mod show;
/// 检查基础设施状态
pub mod status;
