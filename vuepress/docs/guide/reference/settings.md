---
title: Settings
permalink: /reference/settings/
createTime: 2026/09/20 00:44:43
---
# Settings

Harbor 的用户数据统一位于：

```text
~/.harbor/
```

## 目录布局

```text
~/.harbor/
├── core/                       # GUI 管理的 release harbor_core
├── runtime/run/                # 机器级实时运行状态
├── workspace/<workspace-id>/
└── remote/
    └── log/                    # Workspace 独立日志
```

## Workspace 设置

每个 Workspace 至少包含：

- 唯一 id 和显示名称。
- 本地或远端连接类型。
- 一个或多个 `search_paths`。
- 远端模式下的 SSH host、port 与 user。

## 指标刷新

- `performance_metrics_interval_ms`：CPU、网络和 GPU 性能指标的刷新间隔，默认 `1000` 毫秒。
- `resource_metrics_interval_ms`：内存、Swap 和磁盘指标的刷新间隔，默认 `10000` 毫秒。

## 路径规则

- 本地路径在本机解析。
- 远端路径在 SSH 用户的环境中解析。
- `~` 表示对应机器上当前用户的 HOME。
- 扫描最大递归深度为 5。

## 兼容性

GUI 只使用与自身版本、API revision 和 SHA-256 匹配的 `harbor_core`。Core 文件必须是 release 构建；debug 二进制不应进入 `~/.harbor/core/`。
