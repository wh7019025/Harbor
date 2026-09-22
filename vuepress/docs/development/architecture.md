---
title: Harbor 架构
permalink: /development/architecture/
createTime: 2026/09/20 00:44:43
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
└── HTTP API v1 (revision 6)
```

## 数据职责

- YAML 留在用户项目中，是 Task 与 Group 的可版本控制定义。
- Workspace 保存发现范围和独立日志。
- 机器级 runtime 保存跨 Workspace 共享的进程状态。
- GUI 保存界面偏好和远端连接入口，不拥有任务进程。

## 组件职责

### Harbor GUI

Vue 3 前端负责展示 Workspace、Task、Group、日志和 Panel。Tauri command layer 负责本机能力、Core 生命周期和远端 SSH 准备。

GUI 不直接启动用户 Task，也不保存任务运行状态。

### harbor_core

Core 负责扫描 YAML、补齐和校验 UUID、启动进程树、记录日志，并通过 HTTP API 暴露实时状态。

一台机器只允许一个 Core 实例。GUI 退出不会终止 Core 或它已经托管的 Task。

Core 自身退出时同样保留 Task。后续 Core 会读取机器级运行记录并重新接管，因此 GUI 和 Core 都可以独立重启；只有显式停止操作才会终止 Task。

### 项目配置

项目中的 `harbor_taskcfg` 是 Task 与 Group 的事实来源。Harbor 不复制或接管项目文件。

### 用户程序

用户程序是 Core 托管的普通系统进程，通过 stdout/stderr 输出日志。界面能力分为三类：无 UI 不做额外处理；`webview_interface` 由程序提供 HTTP 页面；`vnc_interface` 由远端 Core 为原生窗口建立临时 X11/noVNC 环境。

## 本地连接

本地 GUI 连接 `127.0.0.1:29385`。Core 默认只监听本机，不向局域网开放。

```text
GUI → HTTP API → local harbor_core → Task process
```

## 远端连接

切换远端 Workspace 不产生网络副作用。用户点击连接后，GUI 才通过 SSH 和 HTTP 分阶段检查远端 Core，随后通过远端 HTTP API 操作任务。

```text
GUI ── SSH ──> verify / inspect / optional deploy / start harbor_core
GUI ── HTTP ─> remote harbor_core ──> Task process
```

SSH 不参与持续日志传输和任务控制，它只负责部署与启动 Core。

## 连接原则

GUI 把 Core 视为版本严格匹配的本地服务，而不是向后兼容的公共服务器。GUI 与 Core 必须来自同一次 release 构建，避免维护旧 API 分支。

GUI 使用 `~/.harbor/core/<version>/harbor_core` 中的 release 产物，并校验：

1. Harbor 版本。
2. Core API revision。
3. 构建时固化的 Core SHA-256。

远端连接优先复用运行中的匹配 Core，其次唤醒已经安装的匹配 release，最后才复制对应 release 产物和运行依赖。运行中的 Core 被其他 GUI 占用，或仅能看到存活 PID 而 API 不可达时，GUI 不会强制替换。

## 运行原则

Task UUID 是进程唯一键。同一台机器不允许相同 UUID 同时运行多个实例；Workspace 和项目路径仅决定如何发现定义，不决定进程身份。

Core 为每次 Task 启动记录 leader PID、PGID、session 和启动时刻，并从 `/proc` 读取实际进程成员、程序名与命令。任务状态由 leader 决定，但托管记录会保留到整个进程组退出，因此 leader 先退出时仍能发现和终止残留子进程。运行记录保存在机器级 `~/.harbor/runtime/run/tasks.json`，不放在 `/tmp`；Core 重启后会过滤已经退出的记录，并重新接管仍然存活的进程组。

## 数据位置

```text
project/harbor_taskcfg/                 Task 与 Group 定义
~/.harbor/settings.json                 GUI 与 Workspace 设置
~/.harbor/core/<version>/harbor_core    受管理的 release Core
~/.harbor/runtime/                      机器级运行状态
~/.harbor/workspace/<id>/log/           Workspace 任务日志
~/.harbor/log/harbor.log                Core 日志
~/.agents/skills/harbor/                Harbor Skill
```

安装包中的 `/usr/bin/harbor_core` 只作为受管理 Core 的安装来源。GUI 实际运行的是 `~/.harbor/core/<version>/harbor_core` 中通过 SHA-256 校验的副本。

完整接口见 [Web API](/reference/web-api/)。
