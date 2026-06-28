use tracing::info;

use crate::config::{check_docker, check_docker_compose, Config};
use crate::services::database;
use crate::services::docker;
use crate::services::git;
use crate::services::infra_generator::{self, InfraConfig};
use dialoguer::{Confirm, Input, Password};
use crate::constants::*;

/// 初始化 homespace 环境配置的主入口
#[allow(clippy::cognitive_complexity)]
pub fn run() -> anyhow::Result<()> {
    info!("🔧 homespace - 环境初始化\n");

    // 步骤 1：检查 Docker
    check_docker()?;
    check_docker_compose()?;
    info!("检查 Docker 环境... ✅\n");

    // 步骤 2：加载或创建配置
    let mut config = Config::load_or_default()?;
    let is_first_run = Config::load().is_none();

    if is_first_run {
        info!("首次运行，需要配置基本信息。\n");
    }

    // --- 基础配置 ---
    let apps_root: String = Input::new()
        .with_prompt("应用根目录 (apps_root)")
        .with_initial_text(config.paths.apps_root.display().to_string())
        .interact_text()?;
    config.paths.apps_root = std::path::PathBuf::from(&apps_root);

    let data_root: String = Input::new()
        .with_prompt("数据根目录 (data_root)")
        .with_initial_text(config.paths.data_root.display().to_string())
        .interact_text()?;
    config.paths.data_root = std::path::PathBuf::from(&data_root);

    let domain: String = Input::new()
        .with_prompt("主域名 (如 example.com)")
        .with_initial_text(&config.home.domain)
        .interact_text()?;
    config.home.domain = domain.clone();

    // 提前保存基础配置
    config.save()?;

    // 步骤 3：基础设施设置
    info!("\n━━━ 基础设施初始化 ━━━\n");

    let infra_exists = config.infra_compose_path().exists()
        || config.paths.apps_root.join(INFRA_DIR).join(TRAEFIK_DIR).join(COMPOSE_FILE).exists();

    if infra_exists {
        info!("  ✅ infra 目录已存在，跳过基础设施生成。");
    } else {
        let setup_infra = Confirm::new()
            .with_prompt("是否从零初始化基础设施 (Traefik + PostgreSQL)?")
            .default(true)
            .interact()?;

        if setup_infra {
            let mut infra_cfg = collect_infra_config(&domain, &config)?;

            // Dashboard 用户创建
            let create_user = Confirm::new()
                .with_prompt("\n是否为 Traefik Dashboard 创建认证用户?")
                .default(true)
                .interact()?;

            if create_user {
                let username: String = Input::new()
                    .with_prompt("用户名")
                    .with_initial_text("admin")
                    .interact_text()?;
                let password: String = Password::new()
                    .with_prompt("密码")
                    .with_confirmation("确认密码", "两次输入不一致")
                    .interact()?;

                let entry = crate::services::auth::htpasswd_sha1_entry(&username, &password);
                infra_cfg.dashboard_user = Some((username, entry));
                info!("  ✅ Dashboard 用户已配置");
            }

            info!("\n📁 生成基础设施文件...");
            infra_generator::generate_infra(&infra_cfg)?;

            info!("  ✅ {}/{}/{}", INFRA_DIR, TRAEFIK_DIR, COMPOSE_FILE);
            info!("  ✅ {}/{}/{}", INFRA_DIR, TRAEFIK_DIR, ENV_FILE);
            info!("  ✅ {}/{}/{}/{}", INFRA_DIR, TRAEFIK_DIR, CONFIG_DIR, TLS_CONFIG_FILE);
            info!("  ✅ {}/{}/{}", INFRA_DIR, POSTGRES_DIR, COMPOSE_FILE);
            info!("  ✅ {}/{}/{}", INFRA_DIR, POSTGRES_DIR, ENV_FILE);

            // 创建 Docker 网络
            info!("\n🌐 创建 Docker 网络...");
            for net in &[NETWORK_TRAEFIK, NETWORK_POSTGRES] {
                if docker::network_exists(net).unwrap_or(false) {
                    info!("  ✅ 网络 {} 已存在", net);
                } else {
                    docker::create_network(net)?;
                    info!("  ✅ 网络 {} 已创建", net);
                }
            }

            // Git 操作
            if git::is_git_repo(&config.paths.apps_root) {
                git::git_add(&config.paths.apps_root, &config.paths.apps_root.join(INFRA_DIR))?;
                info!("  ✅ git add {}/", INFRA_DIR);
            }

            // 询问是否启动
            let start_now = Confirm::new()
                .with_prompt("\n是否立即启动基础设施?")
                .default(true)
                .interact()?;

            if start_now {
                info!("\n🚀 启动基础设施...");

                // 先启动 PostgreSQL（Traefik 依赖其网络）
                let pg_dir = config.paths.apps_root.join(INFRA_DIR).join(POSTGRES_DIR);
                info!("  启动 PostgreSQL...");
                docker::compose_up(&pg_dir)?;
                info!("  ✅ PostgreSQL 已启动");

                // 启动 Traefik
                let tr_dir = config.paths.apps_root.join(INFRA_DIR).join(TRAEFIK_DIR);
                info!("  启动 Traefik...");
                docker::compose_up(&tr_dir)?;
                info!("  ✅ Traefik 已启动");

                info!("\n  📊 Traefik Dashboard: https://{}", infra_cfg.service_domain);
                info!("  🗄️  PostgreSQL: localhost:5433");
            } else {
                info!("\n  稍后手动启动:");
                info!("    cd {} && docker compose up -d", config.paths.apps_root.join(INFRA_DIR).join(POSTGRES_DIR).display());
                info!("    cd {} && docker compose up -d", config.paths.apps_root.join(INFRA_DIR).join(TRAEFIK_DIR).display());
            }
        } else {
            info!("  ⚠️  跳过基础设施初始化。");
            info!("  请确保 infra 已手动部署后再添加应用。");

            // 如果网络不存在则创建
            for network in &[NETWORK_TRAEFIK, NETWORK_POSTGRES] {
                if !docker::network_exists(network).unwrap_or(false) {
                    let create = Confirm::new()
                        .with_prompt(format!("Docker 网络 {} 不存在，是否创建?", network))
                        .default(true)
                        .interact()?;
                    if create {
                        docker::create_network(network)?;
                    }
                }
            }
        }
    }

    // 步骤 4：Git 仓库初始化
    info!("\n📝 Git 仓库...");
    if git::is_git_repo(&config.paths.apps_root) {
        info!("  ✅ Git 仓库已存在");
    } else {
        let init_git = Confirm::new()
            .with_prompt(format!(
                "apps_root ({}) 不是 Git 仓库，是否初始化?",
                config.paths.apps_root.display()
            ))
            .default(true)
            .interact()?;
        if init_git {
            git::git_init(&config.paths.apps_root)?;
            info!("  ✅ Git 仓库已初始化");
        }
    }

    // 步骤 5：最终保存
    config.save()?;
    info!("\n✅ homespace 初始化完成!");
    info!("  配置: ~/.config/homespace/config.toml");
    info!("  应用根: {}", config.paths.apps_root.display());
    info!("  域名: {}", config.home.domain);

    Ok(())
}

/// 交互式收集基础设施配置。
fn collect_infra_config(domain: &str, config: &Config) -> anyhow::Result<InfraConfig> {
    info!("配置基础设施:\n");

    // Traefik 配置
    info!("── Traefik ──");

    let traefik_version: String = Input::new()
        .with_prompt("Traefik 版本")
        .with_initial_text("v3.7.5")
        .interact_text()?;

    let service_domain: String = Input::new()
        .with_prompt("Traefik Dashboard 域名")
        .with_initial_text(format!("traefik.{}", domain))
        .interact_text()?;

    let acme_email: String = Input::new()
        .with_prompt("ACME 邮箱 (Let's Encrypt 通知)")
        .with_initial_text(format!("admin@{}", domain))
        .interact_text()?;

    let cf_token: String = Password::new()
        .with_prompt("Cloudflare API Token (Zone:DNS:Edit 权限)")
        .interact()?;

    let http_port: u16 = Input::new()
        .with_prompt("HTTP 端口")
        .with_initial_text("80")
        .interact_text()?;

    let https_port: u16 = Input::new()
        .with_prompt("HTTPS 端口")
        .with_initial_text("443")
        .interact_text()?;

    // PostgreSQL 配置
    info!("\n── PostgreSQL ──");

    let pg_version: String = Input::new()
        .with_prompt("PostgreSQL 版本")
        .with_initial_text("16.14")
        .interact_text()?;

    let pg_user: String = Input::new()
        .with_prompt("管理员用户名")
        .with_initial_text("admin")
        .interact_text()?;

    let auto_password = database::generate_password(20);
    let pg_password: String = Password::new()
        .with_prompt("管理员密码 (回车自动生成)")
        .allow_empty_password(true)
        .interact()?;
    let pg_password = if pg_password.is_empty() {
        info!("  自动生成密码: {}", auto_password);
        auto_password
    } else {
        pg_password
    };

    let pg_db: String = Input::new()
        .with_prompt("默认数据库名")
        .with_initial_text("defaultdb")
        .interact_text()?;

    let timezone: String = Input::new()
        .with_prompt("时区")
        .with_initial_text("Asia/Shanghai")
        .interact_text()?;

    Ok(InfraConfig {
        domain: domain.to_string(),
        acme_email,
        cf_api_token: cf_token,
        service_domain,
        http_port,
        https_port,
        traefik_version,
        postgres_version: pg_version,
        pg_user,
        pg_password,
        pg_db,
        infra_path: config.paths.apps_root.join(INFRA_DIR),
        timezone,
        dashboard_user: None,
    })
}
