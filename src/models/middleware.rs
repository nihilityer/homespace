//! Traefik 文件化中间件定义 — 集中管理中间件的名称、YAML 内容及标签引用方式。
//!
//! 所有中间件均通过 Traefik 文件提供者（`@file`）加载，
//! YAML 文件存放在 `infra/traefik/config/` 目录下。

use serde::{Deserialize, Serialize};

/// Traefik 中间件类型。
///
/// 每个变体对应 `infra/traefik/config/middleware-*.yml` 中的一个文件化中间件定义。
/// 中间件通过 Traefik 文件提供者加载，在路由器标签中以 `name@file` 格式引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Middleware {
    /// GZip 压缩 — 对 HTTP 响应内容进行 gzip 压缩，减少传输数据量。
    Gzip,
    /// HTTP → HTTPS 重定向 — 将所有 HTTP 请求 301 重定向到 HTTPS。
    #[serde(rename = "redir-https")]
    RedirectHttps,
    /// 转发头注入 — 设置 `X-Forwarded-Proto`、`X-Forwarded-Ssl` 和 `X-Forwarded-Port`
    /// 请求头，确保后端应用正确识别前端协议和端口。
    #[serde(rename = "x-forwarded-proto")]
    XForwardedProto,
    /// 内网 IP 白名单 — 仅允许 RFC 1918 私有地址段（及回环地址）访问。
    /// 适用于 Dashboard、管理接口等仅需内网可达的服务。
    #[serde(rename = "ipallowlist-internal")]
    IpAllowlistInternal,
    /// Dashboard 基础认证 — 通过 `users.txt` 文件进行 HTTP Basic Auth 鉴权。
    /// 仅用于 Traefik Dashboard，不适用于普通应用服务。
    #[serde(rename = "dashboard-auth")]
    DashboardAuth,
}

impl Middleware {
    /// 返回中间件在 Traefik 配置中的短名称。
    ///
    /// 该名称同时也是 YAML 文件中 `http.middlewares.<name>` 的键名。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::RedirectHttps => "redir-https",
            Self::XForwardedProto => "x-forwarded-proto",
            Self::IpAllowlistInternal => "ipallowlist-internal",
            Self::DashboardAuth => "dashboard-auth",
        }
    }

    /// 返回 Traefik 标签中的引用字符串，格式为 `<name>@file`。
    ///
    /// 用于 Docker Compose 中 `traefik.http.routers.<name>.middlewares` 标签的值。
    #[must_use]
    pub fn label_ref(self) -> String {
        format!("{}@file", self.name())
    }

    /// 返回该中间件对应的 YAML 文件名。
    ///
    /// 存放于 `infra/traefik/config/` 目录下。
    /// 文件名保持向后兼容，与最初 `infra_generator` 生成的名称一致。
    #[must_use]
    pub fn filename(self) -> &'static str {
        match self {
            Self::Gzip => "middleware-gzip.yml",
            Self::RedirectHttps => "middleware-redir-https.yml",
            Self::XForwardedProto => "middleware-x-forwarded.yml",
            Self::IpAllowlistInternal => "middleware-ipallowlist.yml",
            Self::DashboardAuth => "middleware-dashboard-auth.yml",
        }
    }

    /// 返回该中间件的 YAML 文件完整内容。
    ///
    /// 内容可直接写入磁盘，无需额外处理。
    #[must_use]
    pub fn yaml_content(self) -> &'static str {
        match self {
            Self::Gzip => MIDDLEWARE_GZIP,
            Self::RedirectHttps => MIDDLEWARE_REDIR_HTTPS,
            Self::XForwardedProto => MIDDLEWARE_X_FORWARDED,
            Self::IpAllowlistInternal => MIDDLEWARE_IPALLOWLIST,
            Self::DashboardAuth => MIDDLEWARE_DASHBOARD_AUTH,
        }
    }

    /// 判断该中间件是否适用于普通应用服务。
    ///
    /// 返回 `false` 的中间件仅用于 Traefik 自身（如 Dashboard 认证），
    /// 不会在 `homespace add` 的中间件选择列表中展示。
    #[must_use]
    pub fn app_applicable(self) -> bool {
        !matches!(self, Self::DashboardAuth | Self::RedirectHttps)
    }

    /// 返回所有已定义的中间件变体。
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![
            Self::Gzip,
            Self::RedirectHttps,
            Self::XForwardedProto,
            Self::IpAllowlistInternal,
            Self::DashboardAuth,
        ]
    }

    /// 返回适用于应用服务的中间件列表。
    ///
    /// 等同于 `Self::all()` 过滤掉 `app_applicable()` 返回 `false` 的变体。
    #[must_use]
    pub fn app_applicable_all() -> Vec<Self> {
        Self::all()
            .into_iter()
            .filter(|m| m.app_applicable())
            .collect()
    }

    /// 从名称字符串解析中间件类型。
    ///
    /// 支持带或不带 `@file` 后缀的名称，例如 `"gzip"` 和 `"gzip@file"` 均可解析。
    /// 无法识别的名称返回 `None`。
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        let name = s.strip_suffix("@file").unwrap_or(s);
        match name {
            "gzip" => Some(Self::Gzip),
            "redir-https" => Some(Self::RedirectHttps),
            "x-forwarded-proto" => Some(Self::XForwardedProto),
            "ipallowlist-internal" => Some(Self::IpAllowlistInternal),
            "dashboard-auth" => Some(Self::DashboardAuth),
            _ => None,
        }
    }
}

// ── YAML 内容常量 ────────────────────────────────────────────────

/// GZip 压缩中间件 YAML 内容
const MIDDLEWARE_GZIP: &str = r#"# GZip 压缩 — 对响应内容进行 gzip 压缩
http:
  middlewares:
    gzip:
      compress: {}
"#;

/// HTTP → HTTPS 重定向中间件 YAML 内容
const MIDDLEWARE_REDIR_HTTPS: &str = r#"# HTTP → HTTPS 重定向 — 将所有 HTTP 请求重定向到 HTTPS
http:
  middlewares:
    redir-https:
      redirectScheme:
        scheme: https
        permanent: false
"#;

/// 转发头注入中间件 YAML 内容
const MIDDLEWARE_X_FORWARDED: &str = r#"# 转发头注入 — 确保后端应用正确识别代理协议和端口
http:
  middlewares:
    x-forwarded-proto:
      headers:
        customRequestHeaders:
          X-Forwarded-Proto: "https"
          X-Forwarded-Ssl: "on"
          X-Forwarded-Port: "443"
"#;

/// 内网 IP 白名单中间件 YAML 内容
const MIDDLEWARE_IPALLOWLIST: &str = r#"# 内网 IP 白名单 — 仅允许私有地址段访问（Traefik v3 ipAllowList）
http:
  middlewares:
    ipallowlist-internal:
      ipAllowList:
        sourceRange:
          - "127.0.0.0/8"
          - "10.0.0.0/8"
          - "172.16.0.0/12"
          - "192.168.0.0/16"
"#;

/// Dashboard 基础认证中间件 YAML 内容
const MIDDLEWARE_DASHBOARD_AUTH: &str = r#"# Dashboard 基础认证 — 访问面板需要用户名/密码
# 用户文件: /etc/traefik/config/users.txt
# 添加用户: htpasswd -nB <username> >> config/users.txt
http:
  middlewares:
    dashboard-auth:
      basicAuth:
        usersFile: "/etc/traefik/config/users.txt"
        realm: "Traefik Dashboard"
"#;
