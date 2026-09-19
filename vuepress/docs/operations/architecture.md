---
title: Harbor 架构
permalink: /operations/architecture/
createTime: 2026/09/20 00:21:16
---
# Harbor 架构

![Harbor 系统总览](/images/harbor-overview.svg)

```text
Harbor GUI
├── Vue 3 UI
├── Tauri command layer
└── Core manager
    ├── local: 127.0.0.1:29385
    └── remote: SSH deploy + host:29385

harbor_core
├── Workspace settings
├── YAML discovery and validation
├── UUID registry
├── Process supervisor
├── Runtime state
├── Log storage
└── HTTP API v1 (revision 3)
```

## 数据职责

- YAML 留在用户项目中，是 Task 与 Group 的可版本控制定义。
- Workspace 保存发现范围和独立日志。
- 机器级 runtime 保存跨 Workspace 共享的进程状态。
- GUI 保存界面偏好和远端连接入口，不拥有任务进程。

## 连接原则

GUI 把 Core 视为版本严格匹配的本地服务，而不是向后兼容的公共服务器。GUI 与 Core 必须来自同一次 release 构建，避免维护旧 API 分支。

## 运行原则

Task UUID 是进程唯一键。同一台机器不允许相同 UUID 同时运行多个实例；Workspace 和项目路径仅决定如何发现定义，不决定进程身份。
