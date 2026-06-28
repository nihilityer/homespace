use tracing::info;
use crate::config::Config;
use crate::models::app::{App, DatabaseConfig, NetworkMode, Service, TraefikRoute, Volume};
use crate::services::{docker, generator, git, scanner};
use anyhow::Context;
use dialoguer::Input;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::constants::*;

/// 交互式添加静态网站应用的主入口
pub fn run(config: &Config, commit: bool, no_git: bool, start: bool) -> anyhow::Result<()> {
    info!("🌐 homespace - 添加静态网站应用\n");

    // ===== 第一步：收集用户输入 =====
    info!("━━━ 第一步：应用配置 ━━━\n");

    let existing_apps = scanner::scan_apps(config).unwrap_or_default();

    // 1.1 应用名称
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

    // 1.2 压缩包路径
    let archive_path_str: String = Input::new()
        .with_prompt("静态网站压缩包路径")
        .validate_with(|input: &String| {
            let p = Path::new(input);
            if !p.exists() {
                Err("文件不存在")
            } else if !p.is_file() {
                Err("不是一个文件")
            } else {
                let fname = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                let supported = fname.ends_with(".zip")
                    || fname.ends_with(".tar.gz")
                    || fname.ends_with(".tgz")
                    || fname.ends_with(".tar.bz2")
                    || fname.ends_with(".tar.xz");
                if supported {
                    Ok(())
                } else {
                    Err("不支持的格式，支持: .zip, .tar.gz, .tgz, .tar.bz2, .tar.xz")
                }
            }
        })
        .interact_text()?;
    let archive_path = PathBuf::from(&archive_path_str);

    // 1.3 子域名
    let default_domain = format!("{}.{}", app_name, config.home.domain);

    let domain: String = Input::new()
        .with_prompt("子域名")
        .with_initial_text(&default_domain)
        .validate_with(|input: &String| {
            if input.is_empty() {
                Err("域名不能为空".to_string())
            } else if let Some(conflict) =
                scanner::check_domain_conflict(input, &existing_apps)
            {
                Err(format!("域名冲突: {}", conflict))
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    // 1.4 自定义 nginx 配置
    let custom_nginx: Option<PathBuf> = {
        let path_str: String = Input::new()
            .with_prompt("自定义 nginx 配置文件路径 (可选，回车跳过)")
            .allow_empty(true)
            .interact_text()?;
        if path_str.is_empty() {
            None
        } else {
            let p = PathBuf::from(&path_str);
            if !p.exists() || !p.is_file() {
                anyhow::bail!("自定义 nginx 配置文件不存在: {}", path_str);
            }
            Some(p)
        }
    };

    // ===== 第二步：创建目录并解压 =====
    info!("\n📁 创建应用目录并提取文件...");

    let app_path = config.paths.apps_root.join(&app_name);
    std::fs::create_dir_all(&app_path)
        .with_context(|| format!("无法创建应用目录: {}", app_path.display()))?;

    let site_dir = app_path.join(NGINX_SITE_DIR);
    extract_archive(&archive_path, &site_dir)?;
    info!("  ✅ 文件已提取到: {}", site_dir.display());

    // 统计文件数量
    let file_count = count_files(&site_dir)?;
    info!("  📄 共 {} 个文件", file_count);

    // ===== 第三步：生成 nginx 配置 =====
    info!("\n⚙️  生成 nginx 配置...");
    generate_nginx_config(&app_path, &custom_nginx)?;

    // ===== 第四步：构建 App 结构体 =====
    let app = build_static_app(&app_name, &app_path, &domain);

    // ===== 第五步：生成 compose 文件 =====
    info!("\n📄 生成配置文件...");
    generator::generate_app(&app, config)?;
    info!("  ✅ docker-compose.yml");
    info!("  ✅ .env / .env.example");
    info!("  ✅ .gitignore");
    info!("  ✅ README.md");

    // ===== 第六步：Git 操作 =====
    if !no_git && git::is_git_repo(&config.paths.apps_root) {
        info!("\n📝 Git 操作...");
        git::git_add(&config.paths.apps_root, &app.path)?;
        info!("  ✅ git add {}", app.name);

        if commit {
            let msg = format!("homespace: add-static {}", app.name);
            git::git_commit(&config.paths.apps_root, &msg)?;
            info!("  ✅ git commit -m \"{}\"", msg);
        }
    }

    // ===== 第七步：可选启动 =====
    if start {
        info!("\n🚀 启动应用...");
        docker::compose_up(&app.path)?;
        info!("  ✅ {} 已启动", app.name);
    }

    info!("\n✅ 静态网站 {} 创建完成!", app.name);
    info!("   🌐 访问地址: https://{}", domain);
    if !start {
        info!("   运行以下命令启动:");
        info!("   cd {} && docker compose up -d", app.path.display());
    }

    Ok(())
}

// ── 交互辅助函数 ──

/// 判断压缩包格式并解压到目标目录
fn extract_archive(archive_path: &Path, target_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(target_dir)
        .with_context(|| format!("无法创建目标目录: {}", target_dir.display()))?;

    let fname = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if fname.ends_with(".zip") {
        extract_zip(archive_path, target_dir)
    } else if fname.ends_with(".tar.gz") || fname.ends_with(".tgz") {
        extract_tar(archive_path, target_dir, "z")
    } else if fname.ends_with(".tar.bz2") {
        extract_tar(archive_path, target_dir, "j")
    } else if fname.ends_with(".tar.xz") {
        extract_tar(archive_path, target_dir, "J")
    } else {
        anyhow::bail!(
            "不支持的压缩格式: {}，支持: .zip, .tar.gz, .tgz, .tar.bz2, .tar.xz",
            fname
        );
    }
}

/// 使用 unzip 解压 .zip 文件
fn extract_zip(archive_path: &Path, target_dir: &Path) -> anyhow::Result<()> {
    let status = std::process::Command::new("unzip")
        .arg("-o")
        .arg(archive_path.as_os_str())
        .arg("-d")
        .arg(target_dir.as_os_str())
        .status()
        .context("无法执行 unzip，请确保已安装 (apt install unzip)")?;
    if !status.success() {
        anyhow::bail!("unzip 解压失败");
    }
    Ok(())
}

/// 使用 tar 解压 .tar.* 文件
fn extract_tar(archive_path: &Path, target_dir: &Path, compression: &str) -> anyhow::Result<()> {
    let flag = format!("x{}f", compression);
    let status = std::process::Command::new("tar")
        .arg(&flag)
        .arg(archive_path.as_os_str())
        .arg("-C")
        .arg(target_dir.as_os_str())
        .status()
        .context("无法执行 tar")?;
    if !status.success() {
        anyhow::bail!("tar 解压失败");
    }
    Ok(())
}

/// 递归统计目录中的文件数量
fn count_files(dir: &Path) -> anyhow::Result<usize> {
    let mut count = 0;
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("无法读取目录: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                count += 1;
            } else if path.is_dir() {
                count += count_files(&path)?;
            }
        }
    }
    Ok(count)
}

/// 生成 nginx 配置：有自定义则复制，否则生成默认配置
fn generate_nginx_config(app_dir: &Path, custom: &Option<PathBuf>) -> anyhow::Result<()> {
    if let Some(custom_path) = custom {
        std::fs::copy(custom_path, app_dir.join(NGINX_CONF_FILE)).with_context(|| {
            format!("无法复制 nginx 配置文件: {}", custom_path.display())
        })?;
        info!("  ✅ 自定义 nginx 配置文件已复制");
    } else {
        generate_default_nginx_config(app_dir)?;
    }
    Ok(())
}

/// 写入默认的 nginx 配置
fn generate_default_nginx_config(app_dir: &Path) -> anyhow::Result<()> {
    let config = r#"server {
    listen 80;
    server_name _;

    root /usr/share/nginx/html;
    index index.html index.htm;

    location / {
        try_files $uri $uri/ =404;
    }

    # 静态资源缓存
    location ~* \.(jpg|jpeg|png|gif|ico|css|js|svg|woff2?|ttf|eot)$ {
        expires 30d;
        add_header Cache-Control "public, immutable";
    }

    # 安全头
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
}
"#;
    std::fs::write(app_dir.join(NGINX_CONF_FILE), config)
        .context("无法写入 nginx 配置文件")?;
    info!("  ✅ 默认 nginx 配置文件已生成");
    Ok(())
}

/// 构建静态网站对应的 App 结构体
fn build_static_app(
    app_name: &str,
    app_path: &Path,
    domain: &str,
) -> App {
    App {
        name: app_name.to_string(),
        description: format!("静态网站 — https://{}", domain),
        path: app_path.to_path_buf(),
        data_root: None,
        services: vec![Service {
            name: app_name.to_string(),
            image: NGINX_IMAGE.to_string(),
            command: None,
            internal_ports: vec![80],
            network_mode: NetworkMode::Bridge,
            routes: vec![TraefikRoute {
                subdomain: domain.to_string(),
                path_prefix: None,
            }],
            port_mappings: vec![],
            volumes: vec![
                Volume {
                    host_path: format!("./{}", NGINX_SITE_DIR),
                    container_path: "/usr/share/nginx/html".to_string(),
                    read_only: true,
                },
                Volume {
                    host_path: format!("./{}", NGINX_CONF_FILE),
                    container_path: "/etc/nginx/conf.d/default.conf".to_string(),
                    read_only: true,
                },
            ],
            shared_mounts: vec![],
            env_vars: HashMap::new(),
            database: DatabaseConfig::None,
            middlewares: vec![],
        }],
    }
}
