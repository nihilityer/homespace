use anyhow::Context;
use rand::RngExt;
use std::path::Path;
use std::process::Command;
use crate::constants::*;

/// 生成随机密码
pub fn generate_password(length: usize) -> String {
    let charset: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

/// 从 `infra/postgres/.env` 读取 `PostgreSQL` 管理密码
pub fn read_infra_pg_password(infra_env_path: &Path) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(infra_env_path)
        .with_context(|| "无法读取 infra/postgres/.env 文件")?;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("POSTGRES_PASSWORD=") || line.starts_with("POSTGRES_PASSWORD =") {
            let value = line
                .split_once('=')
                .map(|x| x.1)
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            return Ok(value.to_string());
        }
    }

    anyhow::bail!("infra/postgres/.env 中未找到 POSTGRES_PASSWORD")
}

/// 从 `infra/postgres/.env` 读取 `PostgreSQL` 用户名
pub fn read_infra_pg_user(infra_env_path: &Path) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(infra_env_path).ok().unwrap_or_default();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("POSTGRES_USER=") || line.starts_with("POSTGRES_USER =") {
            let value = line
                .split_once('=')
                .map(|x| x.1)
                .unwrap_or("admin")
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            return Ok(value.to_string());
        }
    }
    Ok("admin".to_string())
}

/// 从 `infra/postgres/.env` 读取 `POSTGRES_DB`（管理连接的目标数据库名）。
///
/// 若文件不存在或未配置则默认返回 `"postgres"`。
pub fn read_infra_pg_db(infra_env_path: &Path) -> String {
    let content = std::fs::read_to_string(infra_env_path).ok().unwrap_or_default();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("POSTGRES_DB=") || line.starts_with("POSTGRES_DB =") {
            return line
                .split_once('=')
                .map(|x| x.1)
                .unwrap_or("postgres")
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
        }
    }
    "postgres".to_string()
}

/// 使用临时容器创建 `PostgreSQL` 数据库和用户。
///
/// `admin_db` 为 psql 连接的目标数据库名（即 infra postgres 容器的 `POSTGRES_DB`）。
/// ```bash
/// docker run --rm --network postgres \
///   -e PGPASSWORD=<admin-pass> \
///   postgres:16 \
///   psql -h postgres -U <admin-user> -d defaultdb \
///   -c "CREATE DATABASE <db>;"
///   -c "CREATE USER <user> WITH PASSWORD '<pass>';"
///   -c "GRANT ALL PRIVILEGES ON DATABASE <db> TO <user>;"
/// ```
pub fn create_database_and_user(
    db_name: &str,
    db_user: &str,
    db_password: &str,
    postgres_network: &str,
    admin_user: &str,
    admin_password: &str,
    admin_db: &str,
) -> anyhow::Result<()> {
    let sql = format!(
        r#"
DO $$ BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_database WHERE datname = '{db}') THEN
        CREATE DATABASE "{db}";
    END IF;
END $$;
DO $$ BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '{user}') THEN
        CREATE USER "{user}" WITH PASSWORD '{pass}';
    END IF;
END $$;
GRANT ALL PRIVILEGES ON DATABASE "{db}" TO "{user}";
\c "{db}"
GRANT ALL ON SCHEMA public TO "{user}";
"#,
        db = db_name,
        user = db_user,
        pass = db_password
    );

    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network",
            postgres_network,
            "-e",
            &format!("PGPASSWORD={}", admin_password),
            "postgres:16",
            "psql",
            "-h",
            CONTAINER_POSTGRES,
            "-U",
            admin_user,
            "-d",
            admin_db,
            "-c",
            &sql,
        ])
        .output()
        .with_context(|| "无法执行临时 PostgreSQL 容器")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "创建数据库/用户失败:\nstdout: {}\nstderr: {}",
            stdout,
            stderr
        );
    }

    Ok(())
}

/// 删除 `PostgreSQL` 数据库和用户
pub fn drop_database_and_user(
    db_name: &str,
    db_user: &str,
    postgres_network: &str,
    admin_user: &str,
    admin_password: &str,
    admin_db: &str,
) -> anyhow::Result<()> {
    let sql = format!(
        r#"
DROP DATABASE IF EXISTS "{db}";
DROP USER IF EXISTS "{user}";
"#,
        db = db_name,
        user = db_user
    );

    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network",
            postgres_network,
            "-e",
            &format!("PGPASSWORD={}", admin_password),
            "postgres:16",
            "psql",
            "-h",
            CONTAINER_POSTGRES,
            "-U",
            admin_user,
            "-d",
            admin_db,
            "-c",
            &sql,
        ])
        .output()
        .with_context(|| "无法执行临时 PostgreSQL 容器")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("删除数据库/用户失败:\nstderr: {}", stderr);
    }

    Ok(())
}

/// 测试 `PostgreSQL` 连接（使用临时容器）
pub fn test_connection(postgres_network: &str) -> anyhow::Result<bool> {
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network",
            postgres_network,
            "postgres:16",
            "pg_isready",
            "-h",
            CONTAINER_POSTGRES,
            "-U",
            "admin",
        ])
        .output();

    match output {
        Ok(out) => {
            let exit = out.status.code().unwrap_or(1);
            Ok(exit == 0)
        }
        Err(_) => Ok(false),
    }
}
