---
title: 端口使用
permalink: /development/ports/
createTime: 2026/09/27 16:12:18
---
# 端口使用

Harbor 将机器级服务固定在少量保留端口上，把用户程序端口留给 Task YAML 声明。固定端口的唯一源码来源是 `harbor_protocol::web_api`；GUI、Core 和测试不应各自重复定义。

## Harbor 固定端口

### `29385` · Core API

`harbor_core` 的管理 API，负责 Workspace、Task、Group、日志、进程和服务状态。

- 本地 Workspace 默认绑定 `127.0.0.1:29385`。
- 远端 Workspace 通常绑定 `0.0.0.0:29385`，GUI 通过远端主机地址直接访问。
- 该接口没有鉴权，只应在可信局域网内使用，不能暴露到公网。
- 一台机器只能运行一个 Core，因此该端口被占用时 Harbor 不会自动改用其他端口。

### `29386` · 托管终端

远端 Core 管理的 `ttyd` 服务端口。

- 只绑定远端 `127.0.0.1:29386`。
- GUI 通过 SSH Tunnel 映射到本机随机回环端口。
- 不需要在远端防火墙开放，也不应由 Task 占用。
- 端口冲突时终端不可用，Core 会明确报告错误，不会选择随机服务端口。

### `29387` · Mobile Web

可选的只读移动端页面。

- 仅在设置中启用 Mobile Web 后监听。
- 固定绑定 `0.0.0.0:29387`，供同一局域网内的手机或浏览器访问。
- 只提供允许公开的运行状态与非本机 Web Panel，不暴露 Core 管理 API。
- 该页面没有独立登录能力，仍然只适用于可信局域网。

### `23682` · 虚拟桌面

共享虚拟桌面的 noVNC HTTP/WebSocket 端口。所有设置 `remote_display_virtual: true` 的远端 Task 共用同一个虚拟显示器。

- 只绑定远端回环地址，由 SSH Tunnel 转发到 GUI 本机随机端口。
- 它不是 TigerVNC RFB 端口；TigerVNC 与 websockify 之间使用 Unix Socket。
- Task YAML 不配置、不监听也不占用该端口。

### `23683` · 真实桌面

远端已登录 X11 真实桌面的 noVNC HTTP/WebSocket 入口。Core 会自动发现实际的 `DISPLAY`。

- 与虚拟桌面使用不同端口，但采用相同的回环监听与 SSH Tunnel 模式。
- 仅用于查看和操作已经存在的真实桌面，不决定 Task 在哪个 Display 中启动。
- 缺少抓屏依赖、没有可用桌面会话或端口被占用时，该入口保持不可用并报告原因。

## Task Web Panel 端口

`webview_interface[].interface_port` 由项目自行选择，不属于 Harbor 固定端口。例如：

```yaml
webview_interface:
  - panel_name: robot_panel
    interface_port: 23681
    localhost_only: false
```

Core 将端口和监听范围注入 Task 环境变量，程序必须读取这些变量启动 HTTP 服务，不要在源码、启动脚本和 YAML 中重复维护端口。

- `localhost_only: true` 时程序应绑定 `127.0.0.1`。
- `localhost_only: false` 时程序应绑定 `0.0.0.0`，远端 GUI 和 Mobile Web 才能访问。
- 同一台机器上同时运行的 Panel 必须使用不同端口。
- 不要选择 `23682`、`23683`、`29385`、`29386` 或 `29387`。

## SSH 与 Tunnel 端口

Workspace 的 SSH 端口默认是 `22`，也可以在 Workspace 中修改。它属于目标机器的 SSH 服务，不是 Harbor 保留端口。

终端和 noVNC 的 Tunnel 在 GUI 本机使用操作系统分配的随机回环端口。随机端口只存在于当前 Tunnel 生命周期中，不写入配置，也不需要用户预留。

```text
GUI 127.0.0.1:<随机端口>
  └── SSH Tunnel ──> remote 127.0.0.1:29386 / 23682 / 23683
```

## 开发端口

Harbor GUI 的 Vite 开发服务器固定使用 `127.0.0.1:1420`，并与 `tauri.conf.json` 中的 `devUrl` 保持一致。这个端口只用于前端热更新，不属于发布版本的运行协议。

VuePress 开发服务器和生产预览工具可以自行选择空闲端口，浏览器中出现的 `4173`、`4174` 等端口不是 Harbor 协议端口，不应写入 Core 或 Task 配置。

Docker 远端测试环境把容器 SSH 映射到 `127.0.0.2:2222`，并把 Core API 映射到 `127.0.0.2:29385`。其中 `2222` 只属于测试环境。

## 修改端口的规则

固定端口需要保持跨组件一致。修改时至少同步检查：

1. `src-tauri/crates/harbor_protocol/src/web_api.rs` 中的协议常量。
2. Core 的监听、服务状态与冲突检测。
3. GUI 的 URL、SSH Tunnel 和默认显示。
4. Docker 远端测试环境及防火墙说明。
5. Harbor Skill、使用指南和本章节。

除非存在明确的协议迁移计划，不要把固定端口改成“冲突时随机选择”。固定失败比连接到错误服务更容易诊断。
