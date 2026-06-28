use crate::models::app::SharedResource;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use crate::constants::*;

/// homespace 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 路径配置
    pub paths: PathsConfig,
    /// 域名配置
    pub home: HomeConfig,
    #[serde(default)]
    /// 共享资源映射
    pub shared_resources: BTreeMap<String, SharedResourceConfig>,
}

/// 路径配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// 应用根目录
    pub apps_root: PathBuf,
    /// 数据根目录
    pub data_root: PathBuf,
}

/// 域名配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeConfig {
    /// 主域名
    pub domain: String,
}

/// config.toml 中 `shared_resources` 的原始形式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedResourceConfig {
    /// 资源路径
    pub path: PathBuf,
    /// 资源描述
    pub description: String,
    #[serde(default)]
    /// 是否只读
    pub read_only: bool,
    /// 文件 UID
    pub uid: Option<u32>,
    /// 文件 GID
    pub gid: Option<u32>,
}

impl Config {
    /// 加载配置，若不存在则返回 None
    pub fn load() -> Option<Self> {
        let path = config_path();
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        toml::from_str(&content).ok()
    }

    /// 加载或创建默认配置
    pub fn load_or_default() -> anyhow::Result<Self> {
        if let Some(cfg) = Self::load() {
            return Ok(cfg);
        }
        // 返回一个未初始化的配置，由 init 命令填写
        Ok(Self {
            paths: PathsConfig {
                apps_root: dirs_home().join("HomeLab"),
                data_root: PathBuf::from("/mnt/data"),
            },
            home: HomeConfig {
                domain: String::from("example.com"),
            },
            shared_resources: BTreeMap::new(),
        })
    }

    /// 保存到 ~/.config/homespace/config.toml
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建配置目录: {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self).context("序列化配置失败")?;
        std::fs::write(&path, content)
            .with_context(|| format!("无法写入配置文件: {}", path.display()))?;
        Ok(())
    }

    /// 从 `SharedResourceConfig` 构建 `SharedResource` 列表
    pub fn get_shared_resources(&self) -> Vec<SharedResource> {
        self.shared_resources
            .iter()
            .map(|(name, cfg)| SharedResource {
                name: name.clone(),
                path: cfg.path.clone(),
                description: cfg.description.clone(),
                read_only: cfg.read_only,
                uid: cfg.uid,
                gid: cfg.gid,
            })
            .collect()
    }

    /// 查找单个共享资源
    #[allow(dead_code)]
    pub fn get_shared_resource(&self, name: &str) -> Option<SharedResource> {
        self.shared_resources.get(name).map(|cfg| SharedResource {
            name: name.to_string(),
            path: cfg.path.clone(),
            description: cfg.description.clone(),
            read_only: cfg.read_only,
            uid: cfg.uid,
            gid: cfg.gid,
        })
    }

    /// 添加共享资源
    pub fn add_shared_resource(&mut self, name: &str, resource: SharedResourceConfig) {
        self.shared_resources
            .insert(name.to_string(), resource);
    }

    /// 删除共享资源
    pub fn remove_shared_resource(&mut self, name: &str) -> bool {
        self.shared_resources.remove(name).is_some()
    }

    /// Traefik 的 docker-compose.yml 路径
    pub fn traefik_compose_path(&self) -> PathBuf {
        self.paths.apps_root.join(TRAEFIK_DIR).join(COMPOSE_FILE)
    }

    /// PostgreSQL 的 docker-compose.yml 路径
    pub fn postgres_compose_path(&self) -> PathBuf {
        self.paths.apps_root.join(POSTGRES_DIR).join(COMPOSE_FILE)
    }

    /// PostgreSQL 的 .env 路径
    pub fn postgres_env_path(&self) -> PathBuf {
        self.paths
            .apps_root
            .join(POSTGRES_DIR)
            .join(ENV_FILE)
    }
}

/// 获取配置文件路径
fn config_path() -> PathBuf {
    dirs_home().join(HOMESPACE_CONFIG_DIR).join(HOMESPACE_CONFIG_FILE)
}

/// 查找 HOME 目录，`$HOME` 未设置时回退到 `/tmp`
fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

/// 确保配置目录存在
#[allow(dead_code)]
pub fn ensure_config_dir() -> anyhow::Result<PathBuf> {
    let dir = dirs_home().join(HOMESPACE_CONFIG_DIR);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("无法创建配置目录: {}", dir.display()))?;
    Ok(dir)
}

/// 检查 docker 是否可用
pub fn check_docker() -> anyhow::Result<()> {
    let output = std::process::Command::new("docker")
        .arg("info")
        .output()
        .context("Docker 未安装或不可用")?;
    if !output.status.success() {
        anyhow::bail!("Docker daemon 未运行或权限不足");
    }
    Ok(())
}

/// 检查 docker compose 是否可用
pub fn check_docker_compose() -> anyhow::Result<()> {
    let output = std::process::Command::new("docker")
        .args(["compose", "version"])
        .output()
        .context("docker compose 不可用")?;
    if !output.status.success() {
        anyhow::bail!("docker compose 不可用");
    }
    Ok(())
}
