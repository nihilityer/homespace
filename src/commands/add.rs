use crate::config::Config;
use crate::constants::*;
use crate::models::app::{
    App, DatabaseConfig, NetworkMode, PortMapping, PortProtocol, Service, SharedResourceMount,
    TraefikRoute, Volume,
};
use crate::models::middleware::Middleware;
use crate::services::database;
use crate::services::docker;
use crate::services::generator;
use crate::services::git;
use crate::services::scanner;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

/// 交互式添加新应用的主入口
#[allow(clippy::cognitive_complexity)]
pub fn run(config: &Config, commit: bool, no_git: bool, start: bool) -> anyhow::Result<()> {
    info!("🚀 homespace - 添加新应用\n");

    // ===== 第一层：应用概要 =====
    info!("━━━ 第一步：应用概要 ━━━\n");

    let app_name: String = Input::new()
        .with_prompt("应用名称 (kebab-case)")
        .validate_with(|input: &String| {
            if input.is_empty() {
                Err("名称不能为空")
            } else if input.contains(' ') {
                Err("名称不能包含空格，请使用 kebab-case")
            } else if config.paths.apps_root.join(input).exists() {
                Err("该目录已存在")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    let description: String = Input::new()
        .with_prompt("应用描述")
        .allow_empty(true)
        .interact_text()?;

    // 数据根路径
    let use_custom_data = Confirm::new()
        .with_prompt(format!(
            "使用全局 data_root ({})?",
            config.paths.data_root.display()
        ))
        .default(true)
        .interact()?;

    let data_root = if use_custom_data {
        None
    } else {
        let custom: String = Input::new()
            .with_prompt("自定义 data_root（如 ./data 或 /mnt/ssd/data）")
            .with_initial_text("./data")
            .interact_text()?;
        Some(PathBuf::from(custom))
    };

    let service_count: usize = Input::new()
        .with_prompt("服务数量")
        .with_initial_text("1")
        .interact_text()?;

    // 扫描已有应用用于冲突检测
    let existing_apps = scanner::scan_apps(config).unwrap_or_default();

    let mut services: Vec<Service> = Vec::new();

    // ===== 第二层：逐服务配置 =====
    for i in 0..service_count {
        info!("\n━━━ 第二步：配置服务 {}/{} ━━━\n", i + 1, service_count);

        let svc_name: String = Input::new()
            .with_prompt("服务名称 (compose service key)")
            .with_initial_text(if service_count == 1 {
                app_name.clone()
            } else {
                format!("{}-svc{}", app_name, i + 1)
            })
            .interact_text()?;

        let image: String = Input::new()
            .with_prompt("Docker 镜像名 (不含 tag)")
            .with_initial_text("library/alpine")
            .interact_text()?;

        let version: String = Input::new()
            .with_prompt("镜像版本 tag")
            .with_initial_text("latest")
            .interact_text()?;

        // 网络模式
        let network_mode = select_network_mode()?;

        // 内部端口
        let ports_str: String = Input::new()
            .with_prompt("容器监听端口 (多个用逗号分隔，如 80,443)")
            .with_initial_text("80")
            .allow_empty(true)
            .interact_text()?;
        let internal_ports: Vec<u16> = if ports_str.is_empty() {
            Vec::new()
        } else {
            ports_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        };

        // 自定义启动命令
        let command = configure_command()?;

        // ===== 第三层：Traefik 路由 =====
        let routes = configure_routes(&app_name, &svc_name, config, &existing_apps)?;

        // ===== 第三层半：Traefik 中间件 =====
        let middlewares = if routes.is_empty() {
            Vec::new()
        } else {
            configure_middleware()?
        };

        // ===== 第四层：端口暴露 =====
        let port_mappings = configure_port_mappings(&existing_apps)?;

        // ===== 第五层：数据库 =====
        let database = configure_database(&svc_name)?;

        // ===== 第六层：持久化与配置 =====
        let (volumes, shared_mounts, env_vars) = configure_storage(config)?;

        services.push(Service {
            name: svc_name,
            image,
            version,
            command,
            internal_ports,
            network_mode,
            routes,
            port_mappings,
            volumes,
            shared_mounts,
            env_vars,
            database,
            middlewares,
        });
    }

    // ===== 第七层：确认与生成 =====
    let app = App {
        name: app_name.clone(),
        description,
        path: config.paths.apps_root.join(&app_name),
        data_root,
        services,
    };

    info!("\n━━━ 配置汇总 ━━━\n");
    print_summary(&app, config);

    let confirmed = Confirm::new()
        .with_prompt("确认生成以上配置?")
        .default(true)
        .interact()?;

    if !confirmed {
        info!("已取消");
        return Ok(());
    }

    // 生成文件
    info!("\n📁 生成文件...");
    generator::generate_app(&app, config)?;
    info!("  ✅ 目录创建完成");
    info!("  ✅ docker-compose.yml");
    info!("  ✅ .env / .env.example");
    info!("  ✅ .gitignore");
    info!("  ✅ README.md");

    // 数据库操作
    for svc in &app.services {
        if let DatabaseConfig::SharedPostgres {
            db_name,
            user,
            auto_create,
        } = &svc.database
            && *auto_create
        {
            info!("\n🗄️  创建数据库...");
            let password = database::generate_password(20);
            let admin_user = database::read_infra_pg_user(&config.postgres_env_path())
                .unwrap_or_else(|_| "admin".to_string());
            let admin_password = database::read_infra_pg_password(&config.postgres_env_path())?;
            let admin_db = database::read_infra_pg_db(&config.postgres_env_path());

            database::create_database_and_user(
                db_name,
                user,
                &password,
                NETWORK_POSTGRES,
                &admin_user,
                &admin_password,
                &admin_db,
            )?;

            info!("  ✅ 数据库 {} 已创建", db_name);
            info!("  ✅ 用户 {} 已创建", user);
            info!("  🔑 密码: {} (请妥善保存)", password);
            info!("  ⚠️  密码已写入 .env 文件");
        }
    }

    // Git 操作
    if !no_git && git::is_git_repo(&config.paths.apps_root) {
        info!("\n📝 Git 操作...");
        git::git_add(&config.paths.apps_root, &app.path)?;
        info!("  ✅ git add {}", app.name);

        if commit {
            let msg = format!("homespace: add {}", app.name);
            git::git_commit(&config.paths.apps_root, &msg)?;
            info!("  ✅ git commit -m \"{}\"", msg);
        }
    }

    // 可选启动
    if start {
        info!("\n🚀 启动应用...");
        docker::compose_up(&app.path)?;
        info!("  ✅ {} 已启动", app.name);
    }

    info!("\n✅ 应用 {} 创建完成!", app.name);
    if !start {
        info!("   运行以下命令启动:");
        info!("   cd {} && docker compose up -d", app.path.display());
    }

    Ok(())
}

/// 配置容器启动命令，回车跳过表示使用镜像默认命令
fn configure_command() -> anyhow::Result<Option<Vec<String>>> {
    use dialoguer::Confirm;
    use tracing::info;

    let has_cmd = Confirm::new()
        .with_prompt("是否需要自定义启动命令? (覆盖镜像默认 CMD)")
        .default(false)
        .interact()?;

    if !has_cmd {
        return Ok(None);
    }

    info!("输入启动命令，每行一个参数 (如 --flag / value)，回车结束:");
    let mut args: Vec<String> = Vec::new();
    loop {
        let arg: String = dialoguer::Input::new()
            .with_prompt("参数")
            .allow_empty(true)
            .interact_text()?;
        if arg.is_empty() {
            break;
        }
        args.push(arg);
    }

    if args.is_empty() {
        Ok(None)
    } else {
        Ok(Some(args))
    }
}

/// 交互式选择 Traefik 文件中间件。
///
/// 仅展示适用于应用服务的中间件（排除 Dashboard 认证和 HTTP 重定向）。
/// 仅在服务配置了 Traefik 路由时调用。
fn configure_middleware() -> anyhow::Result<Vec<Middleware>> {
    let applicable = Middleware::app_applicable_all();
    if applicable.is_empty() {
        return Ok(Vec::new());
    }

    let items: Vec<String> = applicable.iter().map(|m| m.name().to_string()).collect();

    let selections = MultiSelect::new()
        .with_prompt("选择 Traefik 中间件 (空格选择，回车确认)")
        .items(&items)
        .defaults(&vec![false; items.len()])
        .interact()?;

    Ok(selections.into_iter().map(|i| applicable[i]).collect())
}

/// 选择网络模式（bridge / host / 外部网络）
fn select_network_mode() -> anyhow::Result<NetworkMode> {
    let options = &["bridge (默认)", "host", "指定外部网络"];
    let selection = Select::new()
        .with_prompt("网络模式")
        .items(options)
        .default(0)
        .interact()?;

    match selection {
        0 => Ok(NetworkMode::Bridge),
        1 => Ok(NetworkMode::Host),
        2 => {
            let name: String = Input::new()
                .with_prompt("外部网络名 (如 traefik, postgres)")
                .interact_text()?;
            Ok(NetworkMode::External(name))
        }
        _ => Ok(NetworkMode::Bridge),
    }
}

/// 交互式配置 Traefik 路由（子域名、路径前缀、TLS）
fn configure_routes(
    app_name: &str,
    _svc_name: &str,
    config: &Config,
    existing_apps: &[App],
) -> anyhow::Result<Vec<TraefikRoute>> {
    let has_external = Confirm::new()
        .with_prompt("是否需要通过域名对外提供服务?")
        .default(true)
        .interact()?;

    if !has_external {
        return Ok(Vec::new());
    }

    let mut routes = Vec::new();
    let route_count: usize = Input::new()
        .with_prompt("子域名数量")
        .with_initial_text("1")
        .interact_text()?;

    for i in 0..route_count {
        info!("\n  配置路由 {}/{}", i + 1, route_count);
        let default_domain = if route_count == 1 {
            format!("{}.{}", app_name, config.home.domain)
        } else {
            format!("{}-{}.{}", app_name, i + 1, config.home.domain)
        };

        let subdomain: String = Input::new()
            .with_prompt("子域名")
            .with_initial_text(&default_domain)
            .validate_with(|input: &String| {
                if let Some(conflict) = scanner::check_domain_conflict(input, existing_apps) {
                    Err(format!("域名冲突: {}", conflict))
                } else {
                    Ok(())
                }
            })
            .interact_text()?;

        let path_prefix: Option<String> = {
            let prefix: String = Input::new()
                .with_prompt("路径前缀 (可选，如 /api/*，回车跳过)")
                .allow_empty(true)
                .interact_text()?;
            if prefix.is_empty() {
                None
            } else {
                Some(prefix)
            }
        };

        routes.push(TraefikRoute {
            subdomain,
            path_prefix,
        });
    }

    Ok(routes)
}

/// 交互式配置宿主机端口映射
fn configure_port_mappings(existing_apps: &[App]) -> anyhow::Result<Vec<PortMapping>> {
    let has_ports = Confirm::new()
        .with_prompt("是否需要直接暴露端口到宿主机?")
        .default(false)
        .interact()?;

    if !has_ports {
        return Ok(Vec::new());
    }

    let mut mappings = Vec::new();
    let count: usize = Input::new()
        .with_prompt("端口映射数量")
        .with_initial_text("1")
        .interact_text()?;

    for i in 0..count {
        info!("  端口映射 {}/{}", i + 1, count);

        let host_port: u16 = Input::new()
            .with_prompt("宿主机端口")
            .validate_with(|input: &u16| {
                if let Some(conflict) = scanner::check_port_conflict(*input, existing_apps) {
                    Err(format!("端口冲突: {}", conflict))
                } else {
                    Ok(())
                }
            })
            .interact_text()?;

        let container_port: u16 = Input::new().with_prompt("容器端口").interact_text()?;

        let protocol = {
            let options = &["tcp", "udp"];
            let sel = Select::new()
                .with_prompt("协议")
                .items(options)
                .default(0)
                .interact()?;
            match sel {
                0 => PortProtocol::Tcp,
                1 => PortProtocol::Udp,
                _ => PortProtocol::Tcp,
            }
        };

        mappings.push(PortMapping {
            host_port,
            container_port,
            protocol,
        });
    }

    Ok(mappings)
}

/// 交互式配置数据库连接方式。
///
/// 两步决策：先确认是否需要数据库，再确认是否使用 `infra` 共享 `PostgreSQL`。
/// 不使用 `infra` 时，数据库由应用镜像自行处理，不在此建模。
fn configure_database(svc_name: &str) -> anyhow::Result<DatabaseConfig> {
    let need_db = Confirm::new()
        .with_prompt("是否需要数据库?")
        .default(false)
        .interact()?;

    if !need_db {
        return Ok(DatabaseConfig::None);
    }

    let use_infra = Confirm::new()
        .with_prompt("使用 infra 共享 PostgreSQL?")
        .default(true)
        .interact()?;

    if !use_infra {
        // 应用自带数据库，无需 homespace 管理
        return Ok(DatabaseConfig::None);
    }

    let db_name: String = Input::new()
        .with_prompt("数据库名")
        .with_initial_text(svc_name)
        .interact_text()?;
    let user: String = Input::new()
        .with_prompt("数据库用户名")
        .with_initial_text(svc_name)
        .interact_text()?;
    let auto_create = Confirm::new()
        .with_prompt("是否自动创建数据库和用户?")
        .default(true)
        .interact()?;

    Ok(DatabaseConfig::SharedPostgres {
        db_name,
        user,
        auto_create,
    })
}

/// 配置数据卷、共享资源挂载和环境变量
#[allow(clippy::type_complexity)]
fn configure_storage(
    config: &Config,
) -> anyhow::Result<(
    Vec<Volume>,
    Vec<SharedResourceMount>,
    HashMap<String, String>,
)> {
    let mut volumes = Vec::new();
    let mut shared_mounts = Vec::new();
    let mut env_vars = HashMap::new();

    // 共享资源
    let resources = config.get_shared_resources();
    if !resources.is_empty() {
        let mount_shared = Confirm::new()
            .with_prompt("是否需要挂载共享资源目录?")
            .default(true)
            .interact()?;

        if mount_shared {
            let items: Vec<String> = resources
                .iter()
                .map(|r| {
                    format!(
                        "{} ({}, {})",
                        r.name,
                        r.path.display(),
                        if r.read_only { "r/o" } else { "r/w" }
                    )
                })
                .collect();
            let defaults: Vec<bool> = vec![false; items.len()];

            let selections = MultiSelect::new()
                .with_prompt("选择要挂载的共享资源 (空格选择，回车确认)")
                .items(&items)
                .defaults(&defaults)
                .interact()?;

            for idx in selections {
                let res = &resources[idx];

                let container_path: String = Input::new()
                    .with_prompt(format!("资源 {} 在容器内的挂载点", res.name))
                    .with_initial_text(format!("/{}", res.name))
                    .interact_text()?;

                let read_only = if res.read_only {
                    true
                } else {
                    Confirm::new()
                        .with_prompt(format!("以只读方式挂载 {} ?", res.name))
                        .default(false)
                        .interact()?
                };

                shared_mounts.push(SharedResourceMount {
                    resource_name: res.name.clone(),
                    container_path,
                    read_only,
                });

                // 检查 UID/GID 一致性
                if let (Some(uid), Some(gid)) = (res.uid, res.gid) {
                    info!(
                        "  ⚠️  共享资源 {} 的 UID:GID 为 {}:{}，请确保容器内用户匹配",
                        res.name, uid, gid
                    );
                    let set_user = Confirm::new()
                        .with_prompt("是否配置 PUID/PGID 环境变量?")
                        .default(true)
                        .interact()?;
                    if set_user {
                        env_vars.insert("PUID".to_string(), uid.to_string());
                        env_vars.insert("PGID".to_string(), gid.to_string());
                    }
                }
            }
        }
    }

    // 自定义数据卷
    let add_custom_volumes = Confirm::new()
        .with_prompt("是否需要添加自定义数据卷?")
        .default(false)
        .interact()?;

    if add_custom_volumes {
        loop {
            let host_path: String = Input::new()
                .with_prompt("宿主机路径 (回车完成)")
                .allow_empty(true)
                .interact_text()?;

            if host_path.is_empty() {
                break;
            }

            let container_path: String = Input::new().with_prompt("容器内路径").interact_text()?;

            let read_only = Confirm::new()
                .with_prompt("是否只读?")
                .default(false)
                .interact()?;

            volumes.push(Volume {
                host_path,
                container_path,
                read_only,
            });
        }
    }

    // 环境变量
    let add_env = Confirm::new()
        .with_prompt("是否需要添加额外环境变量?")
        .default(false)
        .interact()?;

    if add_env {
        info!("输入环境变量 (格式: KEY=VALUE，回车完成):");
        loop {
            let entry: String = Input::new()
                .with_prompt("KEY=VALUE")
                .allow_empty(true)
                .interact_text()?;

            if entry.is_empty() {
                break;
            }

            if let Some((key, value)) = entry.split_once('=') {
                env_vars.insert(key.trim().to_string(), value.trim().to_string());
            } else {
                info!("  ⚠️  格式错误，请使用 KEY=VALUE 格式");
            }
        }
    }

    Ok((volumes, shared_mounts, env_vars))
}

/// 打印应用配置汇总信息
fn print_summary(app: &App, config: &Config) {
    info!("  应用: {} ({})", app.name, app.description);
    info!("  路径: {}", app.path.display());
    info!(
        "  数据根: {}",
        app.data_root
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| format!("{} (全局)", config.paths.data_root.display()))
    );
    info!("");

    for svc in &app.services {
        info!("  ┌─ Service: {}", svc.name);
        info!("  │  镜像: {}:{}", svc.image, svc.version);
        if !svc.internal_ports.is_empty() {
            let p: Vec<String> = svc.internal_ports.iter().map(|n| n.to_string()).collect();
            info!("  │  端口: {}", p.join(", "));
        }
        match &svc.network_mode {
            NetworkMode::Bridge => info!("  │  网络: bridge"),
            NetworkMode::Host => info!("  │  网络: host"),
            NetworkMode::External(name) => info!("  │  网络: external ({})", name),
        }
        for route in &svc.routes {
            info!("  │  🌐 https://{}", route.subdomain);
        }
        for pm in &svc.port_mappings {
            info!(
                "  │  📡 {}:{}/{:?}",
                pm.host_port, pm.container_port, pm.protocol
            );
        }
        match &svc.database {
            DatabaseConfig::None => {}
            DatabaseConfig::SharedPostgres { db_name, user, .. } => {
                info!("  │  🗄️  infra PG: DB={}, User={}", db_name, user);
            }
        }
        for mount in &svc.shared_mounts {
            info!(
                "  │  📁 [{}] → {}",
                mount.resource_name, mount.container_path
            );
        }
        if !svc.middlewares.is_empty() {
            let mw_names: Vec<&str> = svc.middlewares.iter().map(|m| m.name()).collect();
            info!("  │  ⚙️  Middlewares: {}", mw_names.join(", "));
        }
        info!("");
    }
}
