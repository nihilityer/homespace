use tracing::info;
use crate::config::{Config, SharedResourceConfig};
use anyhow::Context;
use dialoguer::{Confirm, Input, Select};

/// 根据子命令分发到共享资源操作
pub fn run_sub(subcommand: &str, config: &mut Config) -> anyhow::Result<()> {
    match subcommand {
        "list" => list(config),
        "add" => add(config),
        "remove" => remove(config),
        other => anyhow::bail!("未知子命令: {}，支持: list, add, remove", other),
    }
}

/// 列出所有共享资源
fn list(config: &Config) -> anyhow::Result<()> {
    let resources = config.get_shared_resources();

    if resources.is_empty() {
        info!("暂无共享资源定义。");
        info!("使用 homespace resource add 添加。");
        return Ok(());
    }

    info!(
        "  {:<16} {:<40} {:<6} {:<20} {:<}",
        "Name", "Path", "Mode", "UID:GID", "Description"
    );
    info!(
        "  {:-<16} {:-<40} {:-<6} {:-<20} {:-<}",
        "", "", "", "", ""
    );

    for res in &resources {
        let mode = if res.read_only { "r/o" } else { "r/w" };
        let uid_gid = match (res.uid, res.gid) {
            (Some(u), Some(g)) => format!("{}:{}", u, g),
            _ => "-".to_string(),
        };
        info!(
            "  {:<16} {:<40} {:<6} {:<20} {:<}",
            res.name,
            res.path.display().to_string(),
            mode,
            uid_gid,
            res.description
        );
    }

    Ok(())
}

/// 交互式添加共享资源
fn add(config: &mut Config) -> anyhow::Result<()> {
    let name: String = Input::new()
        .with_prompt("资源名称 (kebab-case)")
        .interact_text()?;

    if config.shared_resources.contains_key(&name) {
        anyhow::bail!("资源 {} 已存在", name);
    }

    let path: String = Input::new()
        .with_prompt("宿主机路径")
        .interact_text()?;

    let description: String = Input::new()
        .with_prompt("描述")
        .interact_text()?;

    let read_only = Confirm::new()
        .with_prompt("是否只读挂载?")
        .default(false)
        .interact()?;

    let uid: Option<u32> = {
        let uid_str: String = Input::new()
            .with_prompt("目录所属 UID (回车跳过)")
            .allow_empty(true)
            .interact_text()?;
        if uid_str.is_empty() {
            None
        } else {
            Some(uid_str.parse().context("UID 必须为数字")?)
        }
    };

    let gid: Option<u32> = if uid.is_some() {
        let gid_str: String = Input::new()
            .with_prompt("目录所属 GID (回车跳过)")
            .allow_empty(true)
            .interact_text()?;
        if gid_str.is_empty() {
            None
        } else {
            Some(gid_str.parse().context("GID 必须为数字")?)
        }
    } else {
        None
    };

    config.add_shared_resource(
        &name,
        SharedResourceConfig {
            path: std::path::PathBuf::from(&path),
            description,
            read_only,
            uid,
            gid,
        },
    );

    config.save()?;
    info!("✅ 共享资源 {} 已添加并保存", name);

    Ok(())
}

/// 交互式删除共享资源
fn remove(config: &mut Config) -> anyhow::Result<()> {
    let resources = config.get_shared_resources();
    if resources.is_empty() {
        info!("暂无共享资源可删除");
        return Ok(());
    }

    let names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    let selection = Select::new()
        .with_prompt("选择要删除的资源")
        .items(&names)
        .interact()?;

    let name = names[selection];
    let confirmed = Confirm::new()
        .with_prompt(format!("确认删除共享资源 {}?", name))
        .default(false)
        .interact()?;

    if confirmed {
        config.remove_shared_resource(name);
        config.save()?;
        info!("✅ 共享资源 {} 已删除", name);
    } else {
        info!("已取消");
    }

    Ok(())
}
