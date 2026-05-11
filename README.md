# sock5s

一个使用 Rust 编写的轻量级 SOCKS5 代理服务。

## 功能特性

- ✅ 兼容 RFC 1928（SOCKS5）
- ✅ 支持认证方式
  - ✅ 无认证（NO AUTHENTICATION REQUIRED）
  - ✅ 用户名密码认证（RFC 1929）
- ✅ `CONNECT` 命令
  - ✅ IPv4
  - ✅ IPv6
  - ✅ 域名
- ✅ `UDP ASSOCIATE` 命令
  - ✅ IPv4
  - ✅ IPv6
  - ✅ 域名
- ✅ 双栈支持（IPv4 / IPv6）
- ✅ 基于 Tokio 的异步实现
- ✅ 跨平台（Linux / macOS / Windows）

## 配置文件

程序通过 TOML 配置文件启动，示例见 [config.toml.example](./config.toml.example)。

示例：

```toml
listen = "0.0.0.0:1080"

[auth]
users = [
  { username = "admin", password = "change-me" }
]

[access]
mode = "blacklist"
ips = ["127.0.0.1", "10.0.0.5"]
cidrs = ["192.168.0.0/16", "172.16.10.0/24"]

[log]
dir = "logs"
retention_days = 7
max_file_size_mb = 100
```

说明：

- `listen`：监听地址。
- `auth.users`：
- 留空时，使用无认证模式（NO AUTH）。
- 非空时，启用用户名密码认证（RFC 1929），客户端需提供有效账号密码。
- `access.mode`：访问控制模式，支持 `blacklist` 和 `whitelist`。
- `access.ips`：目标 IP 列表。
- `access.cidrs`：目标网段列表，支持 CIDR。
- 当 `mode = "blacklist"` 时，命中的 IP/网段会被拒绝。
- 当 `mode = "whitelist"` 时，只有命中的 IP/网段允许访问，其余全部拒绝。
- `log.dir`：日志目录。
- `log.retention_days`：保留最近多少天日志，默认 `7`。
- `log.max_file_size_mb`：单个日志文件大小上限，默认 `100` MB；超过后自动新建下一个文件。

日志内容包括：

- 代理协议（TCP / UDP）
- 代理用户名
- 客户端 IP
- 客户端地址
- 访问的目标 IP:端口
- 被访问控制拦截的目标 IP
- 认证成功事件
- 连接关闭事件
- 认证失败事件与失败原因
- 目标连接失败事件与失败原因

## 使用方式

```bash
sock5s --config ./config.toml
```

命令帮助：

```text
Usage: sock5s --config <FILE>

Options:
  -c, --config <FILE>  Path to TOML configuration file
  -h, --help           Print help
  -V, --version        Print version
```

## 许可证

本项目基于 [MIT license] 发布。

[MIT license]: https://github.com/nanpuyue/sock5s/blob/main/LICENSE

## 项目主页

[https://github.com/nanpuyue/sock5s](https://github.com/nanpuyue/sock5s)
