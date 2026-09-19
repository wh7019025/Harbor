---
title: workspaces
createTime: 2026/09/19 23:55:58
permalink: /guide/ivvg79ut/
---
# 工作空间

## 隔离边界

每个 Workspace 拥有独立的：

- `search_paths`：要扫描的本地或远端目录。
- 日志目录：`~/.harbor/workspace/<workspace-id>/log`。
- 连接配置：本机或 SSH 远端。

运行状态不按 Workspace 隔离。机器级运行信息存放在 `~/.harbor/runtime/run`，从而保证同一 UUID 在同一机器上只有一个实例。

## 本地 Workspace

本地 Workspace 连接本机 Core。通常只监听 `127.0.0.1:29385`，不会暴露到局域网。

## 远端 Workspace

远端 Workspace 通过 SSH 探测和部署 Core，随后 GUI 直接访问远端 Core 的 HTTP 地址。搜索路径和日志路径都按远端用户的 HOME 解释。

## 搜索路径建议

- 添加具体项目目录或少量项目的共同父目录。
- 避免直接扫描 HOME 或大型挂载盘。
- 路径输入支持 `~` 展开与最多 50 条补全候选。
- 一个项目的配置目录必须命名为 `harbor_taskcfg`。

标准结构：

```text
project/
├── harbor_taskcfg/
│   ├── tasks/
│   │   └── driver.yaml
│   └── groups/
│       └── bringup.yaml
└── ...
```
