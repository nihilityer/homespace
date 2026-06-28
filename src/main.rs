//! homespace - `HomeLab` 应用管理 CLI
//!
//! 管理 `HomeLab` 环境中的 Docker Compose 应用的生命周期。

/// 子命令模块
mod commands;
/// 配置加载与持久化
mod config;
/// 常量定义
mod constants;
/// 数据模型
mod models;
/// 核心服务逻辑
mod services;

use clap::{Parser, Subcommand};
use config::Config;
use tracing::error;

/// 🏠 homespace - `HomeLab` 应用管理 CLI
///
/// 管理 `HomeLab` 环境中的 Docker Compose 应用：
/// - 盘点现有应用
/// - 交互式添加新应用
/// - 修改已有应用配置
/// - 标准化目录结构和部署
#[derive(Parser)]
#[command(name = "homespace", version, about)]
struct Cli {
    #[command(subcommand)]
    /// 子命令
    command: Commands,
    /// 自动执行 git commit
    #[arg(long, global = true)]
    commit: bool,
    /// 跳过 Git 操作
    #[arg(long, global = true)]
    no_git: bool,
}

/// 支持的 CLI 子命令
#[derive(Subcommand)]
enum Commands {
    /// 初始化 homespace 环境配置
    Init,
    /// 列出所有应用及其状态
    List,
    /// 启动应用
    Start {
        /// 应用名称
        app: String,
    },
    /// 停止应用
    Stop {
        /// 应用名称
        app: String,
    },
    /// 重启应用
    Restart {
        /// 应用名称
        app: String,
    },
    /// 拉取应用的最新镜像
    Pull {
        /// 应用名称
        app: String,
    },
    /// 构建应用镜像
    Build {
        /// 应用名称
        app: String,
    },
    /// 查看应用详细配置
    Show {
        /// 应用名称
        app: String,
    },
    /// 交互式添加新应用
    Add {
        /// 创建后自动启动
        #[arg(long)]
        start: bool,
    },
    /// 交互式添加静态网站应用
    AddStatic {
        /// 创建后自动启动
        #[arg(long)]
        start: bool,
    },
    /// 修改已有应用配置
    Edit {
        /// 应用名称
        app: String,
    },
    /// 移除应用
    Remove {
        /// 应用名称
        app: String,
        /// 跳过确认提示
        #[arg(long)]
        force: bool,
        /// 同时删除数据目录
        #[arg(long)]
        purge: bool,
    },
    /// 检查基础设施状态
    Status,
    /// 管理共享资源目录
    Resource {
        #[command(subcommand)]
        /// 资源操作子命令
        action: ResourceCmd,
    },
}

/// 共享资源操作子命令
#[derive(Subcommand)]
enum ResourceCmd {
    /// 列出所有共享资源
    List,
    /// 添加共享资源
    Add,
    /// 删除共享资源
    Remove,
}

fn main() {
    // 初始化 tracing subscriber，输出到 stdout
    tracing_subscriber::fmt()
        .with_writer(std::io::stdout)
        .with_target(false)
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        error!("❌ 错误: {}", e);
        // 输出错误链
        let mut source = e.source();
        while let Some(s) = source {
            error!("   原因: {}", s);
            source = std::error::Error::source(s);
        }
        std::process::exit(1);
    }
}

/// 根据 CLI 参数分发到对应子命令
fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Init => {
            commands::init::run()
        }
        Commands::List => {
            let config = Config::load_or_default()?;
            commands::list::run(&config)
        }
        Commands::Start { app } => {
            let config = Config::load_or_default()?;
            commands::lifecycle::run(&app, &config, "start", cli.commit, cli.no_git)
        }
        Commands::Stop { app } => {
            let config = Config::load_or_default()?;
            commands::lifecycle::run(&app, &config, "stop", cli.commit, cli.no_git)
        }
        Commands::Restart { app } => {
            let config = Config::load_or_default()?;
            commands::lifecycle::run(&app, &config, "restart", cli.commit, cli.no_git)
        }
        Commands::Pull { app } => {
            let config = Config::load_or_default()?;
            commands::lifecycle::run(&app, &config, "pull", cli.commit, cli.no_git)
        }
        Commands::Build { app } => {
            let config = Config::load_or_default()?;
            commands::lifecycle::run(&app, &config, "build", cli.commit, cli.no_git)
        }
        Commands::Show { app } => {
            let config = Config::load_or_default()?;
            commands::show::run(&app, &config)
        }
        Commands::Add { start } => {
            let config = Config::load_or_default()?;
            commands::add::run(&config, cli.commit, cli.no_git, start)
        }
        Commands::AddStatic { start } => {
            let config = Config::load_or_default()?;
            commands::add_static::run(&config, cli.commit, cli.no_git, start)
        }
        Commands::Edit { app } => {
            let config = Config::load_or_default()?;
            commands::edit::run(&app, &config, cli.commit, cli.no_git)
        }
        Commands::Remove {
            app,
            force,
            purge,
        } => {
            let config = Config::load_or_default()?;
            commands::remove::run(&app, &config, force, purge, cli.no_git, cli.commit)
        }
        Commands::Status => {
            let config = Config::load_or_default()?;
            commands::status::run(&config)
        }
        Commands::Resource { action } => {
            let mut config = Config::load_or_default()?;
            let sub = match action {
                ResourceCmd::List => "list",
                ResourceCmd::Add => "add",
                ResourceCmd::Remove => "remove",
            };
            commands::resource::run_sub(sub, &mut config)
        }
    }
}
