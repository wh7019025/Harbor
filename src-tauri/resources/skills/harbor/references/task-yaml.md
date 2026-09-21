# Task YAML

完整示例（Agent 直接写**新**文件时，`version` 原样设为 `harbor --version` 的输出；修改已有文件且 version 已正确时不必改）：

```yaml
version: "0.2.0-preview.3"
uuid: a35b7f18-9d64-4e2a-8f31-6c0d72b94511
id: demo-ping
name: Demo Ping
description: 演示用的周期性 hello ping
workdir: $(harbor_taskcfg_dir)/..
env: {}
configs:
  - id: development
    name: 开发环境
    env:
      LOG_LEVEL: debug
  - id: production
    name: 生产环境
    env:
      LOG_LEVEL: info
      WORKERS: "4"
default_config: development
sudo: false
command:
  shell: sh
  script: |
    echo hello
    sleep 1
```

或 argv 形式：

```yaml
version: "0.2.0-preview.3"
uuid: 7f2a61c4-3e98-4b57-b026-d14c9a835e60
id: uname-kernel
name: Uname Kernel
description: ""
workdir: "~"
env: {}
sudo: false
command:
  argv:
    - uname
    - -a
```

## 字段

| 字段 | 必填 | 说明 |
|------|------|------|
| `version` | 是 | Harbor **应用**版本（`harbor --version` 的原样输出），不是配置修订号。**API 保存时自动维护**；Agent 直接编辑时仅在旧值或与应用不一致时更新，**禁止自行递增 rc 号**。详见 [version.md](version.md) |
| `uuid` | 是 | Task 的稳定运行身份。缺失时 harbor_core 自动生成并写回；不要复制其他 YAML 的 UUID，冲突时通过 Harbor 重置其中一个 |
| `id` | 是 | 字母数字、`-`、`_`；同一 `tasks/` 目录内唯一 |
| `name` | 否 | 显示名；省略或空时 UI 显示 `id` |
| `description` | 否 | 任务说明（可为 `""`）。**Agent 编写时尽量用中文**，简要说明任务用途；无说明时可留空 |
| `workdir` | 是 | 启动时工作目录，不可为空。见下文 |
| `env` | 否 | 环境变量 map，默认 `{}`；Harbor 会自动继承当前用户图形会话的 `DISPLAY`、`XAUTHORITY`、Wayland 与 DBus 环境，显式填写时以 YAML 为准 |
| `configs` | 否 | 有序的运行配置列表；每项可覆盖顶层 `env` |
| `configs[].id` | 是 | Task 内唯一，字符规则与 Task `id` 相同 |
| `configs[].name` | 否 | UI 显示名；省略或为空时显示 config `id` |
| `configs[].env` | 否 | 此 config 的环境变量，默认 `{}` |
| `default_config` | 否 | 默认 config `id`；省略时使用 `configs` 第一项 |
| `sudo` | 否 | 默认 `false`；为 `true` 时启动需输入密码 |
| `webview_interface` | 否 | 程序自己提供的 Web 页面列表 |
| `vnc_interface` | 否 | 远端桌面程序的 VNC 页面；最多一个 |
| `command` | 是 | 执行方式：`argv` **或** `shell` + `script` 二选一 |

## command

- **argv**：直接执行，第一项为可执行文件
- **shell + script**：等价于 `{shell} -lc {script}`

二者不可同时为空。

## configs

- `configs` 不存在或为空时，Task 行为与旧配置完全一致。
- UI 仅在一个 Task 有多个 config 时显示选择栏；没有 config 或只有一个时保持紧凑单行。
- 运行时先加载顶层 `env`，再用所选 `configs[].env` 覆盖同名变量。
- `default_config` 若填写，必须引用当前 Task 中已有的 config；未填写时选择列表第一项。
- 同一 Task 同时只能运行一个 config。切换选择后，下一次启动或重启使用新 config。
- Group 还可以通过 `tasks[].env` 再次覆盖 config 环境变量，完整顺序为：`task.env` → `config.env` → `group task env`。

## 选择界面模式

每个 Task 先按程序本身选择模式，与 local/remote 无关：

- **无 UI**：不配置 `webview_interface` 和 `vnc_interface`。服务、脚本、ROS 2 节点和 headless 程序默认使用这种方式。
- **WebView**：程序自身提供 HTTP 页面时配置 `webview_interface`。
- **VNC**：程序只有原生 X11/Qt/GTK 窗口，并且远端需要操作它时配置 `vnc_interface`。

远端 Task 不等于图形 Task。没有 UI 的远端程序不要配置 VNC。禁止使用旧字段
`panel_interface` 和 `force_display`。

## description

- **尽量用中文**写一句简短说明，描述任务做什么、在什么场景下用。
- 用户未提供且一时无法概括时可写 `""`，但不要用英文占位敷衍。
- `name` 可以是英文标识风格；`description` 面向人读，优先中文。

## webview_interface

可选。任务对外提供的网页面板，例如机器人控制页。Harbor 用这里的端口拼 URL，任务 **running** 后可打开。

```yaml
webview_interface:
  - panel_name: robot_panel
    interface_port: 23842
    localhost_only: false
```

| 字段 | 说明 |
|------|------|
| `panel_name` | 面板 id，字符规则与 Task `id` 相同；同一 Task 内唯一 |
| `interface_port` | 面板 HTTP 端口，不能为 0 |
| `localhost_only` | 默认 `false`。`true` 表示面板只绑 127.0.0.1 |
local workspace 打开 `http://127.0.0.1:<port>/`；remote 打开 `http://<ssh.host>:<port>/`。

Harbor 启动 Task 时会把单个面板声明注入以下保留环境变量，程序应读取它们，
不要在 `env` 或源码中重复保存端口：

- `HARBOR_WEBVIEW_NAME`
- `HARBOR_WEBVIEW_INTERFACE_PORT`
- `HARBOR_WEBVIEW_LOCALHOST_ONLY`

每个面板还会获得带标准化面板名的变量，例如
`robot-panel` 对应 `HARBOR_WEBVIEW_ROBOT_PANEL_INTERFACE_PORT`。多面板 Task 应读取
带面板名的变量；只有单面板 Task 会获得不带面板名的三个快捷变量。

## vnc_interface

可选。用于无法提供 Web 页面的桌面程序，最多声明一个：

```yaml
vnc_interface:
  - panel_name: desktop
    interface_port: 23682
```

local workspace 会忽略该入口并直接打开原生窗口。remote workspace 会建立隔离的
X11、TigerVNC 和 noVNC 环境，并在 `interface_port` 发布桌面页面。程序不会收到
VNC 相关环境变量，也不需要知道 Harbor 的显示配置。

使用 `vnc_interface` 时不要在 `env` 或启动脚本中设置 `DISPLAY`、`XAUTHORITY`、
`WAYLAND_DISPLAY`、`QT_QPA_PLATFORM` 或 `GDK_BACKEND`。远端由 Harbor 设置隔离显示
环境，本地则继承当前图形会话；手写 `DISPLAY=:0` 会绕过远端 VNC。

`interface_port` 是浏览器访问 noVNC 的 HTTP/WebSocket 端口，不是 VNC TCP 端口。
TigerVNC 的 TCP 监听会被关闭，只通过 Core 创建的临时 Unix Socket 通信。

远端机器需要安装 `tigervnc-standalone-server`、`novnc`、`websockify` 和 `openbox`。
Harbor 会在启动 Task 前检查这些依赖；缺失时启动失败并直接返回缺失项以及适用于
Debian/Ubuntu 的安装命令，同时把同一错误写入该次 Task Log。

## workdir

- `"~"` 或 `"~/..."`：当前用户 HOME。必须加引号；YAML 会把未加引号的 `~` 解析为 `null`
- `null`：等价于 `$(harbor_taskcfg_dir)`，表示当前 Task 所属的 `harbor_taskcfg` 目录
- 绝对路径
- 相对路径：相对于**项目根**（该 YAML 所在 `harbor_taskcfg` 的**父目录**）
- `$(harbor_taskcfg_dir)`：当前 `harbor_taskcfg` 目录；项目任务常用 `$(harbor_taskcfg_dir)/..` 指向仓库根

## 运行时身份

- 跨工程允许同名 `id`；同一 `tasks/` 目录内短 id 仍唯一
- 运行时身份为 `uuid`；同一台机器同一时间只能运行一个相同 UUID 的实例
- workspace 切换不会停止任务；新 workspace 发现同一 UUID 时会显示同一运行状态

## 不要写入 YAML

`folder`、`prefix_path`、`taskcfg_dir` 由 Harbor 根据文件位置自动填充。
