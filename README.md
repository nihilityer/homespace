# 🏠 homespace

**Homelab Docker Compose 应用管理 CLI**

交互式管理 Homelab 环境中的 Docker Compose 应用：Traefik 反向代理 + PostgreSQL 基础设施的一站式搭建，应用的增删改查与状态监控。

## 功能

- **环境初始化** — 一键搭建 Traefik（边缘路由 + TLS）+ PostgreSQL 共享数据库
- **应用管理** — 交互式添加、编辑、列表、查看、移除 Docker Compose 应用
- **自动生成** — 每个应用自动生成 `docker-compose.yml`、`.env`、`.env.example`、`.gitignore`、`README.md`
- **Traefik 集成** — 自动配置路由、TLS（Cloudflare DNS-01）、中间件（gzip、IP 白名单等）
- **数据库** — 支持连接 infra 共享 PostgreSQL，自动创建数据库和用户
- **状态监控** — 扫描所有应用运行状态，检测端口和域名冲突
- **Git 版本控制** — 可选的自动 commit，所有配置纳入版本管理

## 安装

### 从源码构建

```bash
git clone https://git.nihilityer.top/HomeLab/automation.git
cd automation
cargo build --release
cp target/release/homespace ~/.local/bin/
```

### 前置依赖

- [Docker](https://docs.docker.com/engine/install/) ≥ 20.10
- [Docker Compose](https://docs.docker.com/compose/install/) ≥ 2.0
- [Git](https://git-scm.com/)（可选，用于版本控制）

## 快速开始

```bash
# 1. 初始化环境（配置域名、Traefik、PostgreSQL）
homespace init

# 2. 启动基础设施
cd ~/HomeLab/infra && docker compose up -d

# 3. 添加第一个应用
homespace add

# 4. 查看所有应用状态
homespace list

# 5. 查看基础设施状态
homespace status
```

## CLI 参考

```
🏠 homespace - HomeLab 应用管理 CLI

Usage: homespace [OPTIONS] <COMMAND>

Commands:
  init      初始化 homespace 环境配置
  list      列出所有应用及其状态
  show      查看应用详细配置
  add       交互式添加新应用
  edit      修改已有应用配置
  remove    移除应用
  status    检查基础设施状态
  resource  管理共享资源目录
  help      Print this message or the help of the given subcommand(s)

Options:
      --commit  自动执行 git commit
      --no-git  跳过 Git 操作
  -h, --help    Print help
  -V, --version Print version
```

## 目录结构

```
~/HomeLab/                          # apps_root（默认）
├── infra/                          # 基础设施
│   ├── traefik/                    #   Traefik 边缘路由
│   │   ├── docker-compose.yml
│   │   ├── .env
│   │   └── config/                 #     TLS、中间件 YAML
│   └── postgres/                   #   共享 PostgreSQL
│       ├── docker-compose.yml
│       ├── .env
│       └── data/
├── app1/                           # 应用 1
│   ├── docker-compose.yml
│   ├── .env
│   ├── .env.example
│   ├── .gitignore
│   └── README.md
└── app2/                           # 应用 2
    └── ...
```

## 中间件

应用可通过 Traefik 文件中间件增强安全性，支持：

| 中间件 | 说明 | 适用场景 |
|--------|------|----------|
| `gzip` | GZip 压缩 | 所有 HTTP 响应 |
| `x-forwarded-proto` | 转发代理头注入 | 后端需识别协议 |
| `ipallowlist-internal` | 内网 IP 白名单 | 管理面板、API |

## License

MIT
