---
title: Task YAML
permalink: /reference/task-yaml/
createTime: 2026/09/20 00:44:43
---
# Task YAML

## 完整示例

```yaml
version: "0.2.0"
uuid: a35b7f18-9d64-4e2a-8f31-6c0d72b94511
id: demo-ping
name: Demo Ping
description: 演示周期性输出
workdir: $(harbor_taskcfg_dir)/..
env:
  BASE_MODE: normal
configs:
  - id: development
    name: 开发环境
    env:
      LOG_LEVEL: debug
  - id: production
    name: 生产环境
    env:
      LOG_LEVEL: info
default_config: development
sudo: false
webview_interface:
  - panel_name: status
    interface_port: 23681
    localhost_only: false
command:
  shell: bash
  script: |
    python3 -u main.py
```

## 顶层字段

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `version` | 是 | Harbor 应用版本；API 保存时自动维护。 |
| `uuid` | 自动 | 稳定运行身份；缺失时 Core 自动生成并写回。 |
| `id` | 是 | 同一 `tasks/` 目录内唯一，只使用字母、数字、`-`、`_`。 |
| `name` | 否 | 界面显示名，缺失时显示 `id`。 |
| `description` | 否 | 面向用户的任务说明。 |
| `workdir` | 是 | 进程工作目录。 |
| `env` | 否 | 顶层环境变量 map。 |
| `configs` | 否 | 可选运行配置列表。 |
| `default_config` | 否 | 默认 config id。 |
| `sudo` | 否 | 是否通过 sudo 启动，默认 `false`。 |
| `webview_interface` | 否 | 程序自己提供的 Web 页面列表。 |
| `remote_display_virtual` | 否 | `true` 时将远端原生窗口放入 Harbor 共享虚拟桌面，默认 `false`。 |
| `command` | 是 | `argv` 或 `shell` + `script`。 |

## command

以下两种形式二选一：

```yaml
command:
  argv: [uname, -a]
```

```yaml
command:
  shell: bash
  script: |
    echo hello
```

脚本形式等价于执行 `{shell} -lc {script}`。

## configs

每项包含唯一 `id`、可选 `name` 和可选 `env`。运行时环境覆盖顺序为：

```text
env → configs[].env → Group tasks[].env
```

## workdir

- `"~"` 或 `"~/..."`：当前用户 HOME。
- `null`：当前 Task 所属 `harbor_taskcfg`。
- 绝对路径：直接使用。
- 相对路径：相对于项目根，即 `harbor_taskcfg` 的父目录。
- `$(harbor_taskcfg_dir)`：当前配置目录变量。

`folder`、`prefix_path`、`taskcfg_dir` 是扫描结果，不应写入 YAML。

## Web Panel

`webview_interface` 可以声明一个或多个由程序自身提供的 HTTP 页面：

```yaml
webview_interface:
  - panel_name: status
    interface_port: 23681
    localhost_only: false
```

`interface_port` 由项目选择，但不能与 Harbor 固定端口或同机其他程序冲突。
`localhost_only` 默认为 `false`；程序应读取 Harbor 注入的 `HARBOR_WEBVIEW_*`
环境变量决定端口和监听地址，不要在源码中再次固定。完整端口规则见
[端口使用](/development/ports/)。

## 桌面程序可视化

Qt、GTK 等桌面程序无需改造成 Web 应用，只需声明 VNC 接口：

```yaml
remote_display_virtual: true
command:
  argv: [your-gui-program]
```

`remote_display_virtual` 是布尔值，不配置名称或端口。local workspace 仍然直接打开原生
窗口，不启动 VNC，也不显示该 Panel。remote workspace 会按需启动当前机器唯一的
共享 X11、TigerVNC 和 noVNC 桌面，并在 Harbor 固定端口 `23682` 提供入口。一个
机器上的所有远端虚拟显示 Task 共用同一个 `DISPLAY`。

不要在这类 Task 的 `env` 或启动脚本中手写 `DISPLAY`、`XAUTHORITY`、
`WAYLAND_DISPLAY`、`QT_QPA_PLATFORM` 或 `GDK_BACKEND`。远端显示环境由 Harbor
管理，本地则继承当前图形会话；手工固定 `DISPLAY` 会绕过 VNC。

停止一个 Task 只关闭该程序，不会关闭共享桌面，也不会影响桌面中的其他 Task。
TigerVNC 不监听 TCP，只使用 Harbor 运行时目录中的 Unix Socket。

远端机器至少需要安装 `tigervnc-standalone-server`、`novnc`、`websockify` 和
`util-linux`。Openbox 桌面还需要 `openbox`；GNOME 桌面需要 `ubuntu-session`、
`gnome-session`、`gnome-shell`、`gnome-session-flashback` 和 `dbus-x11`。
Harbor 会在启动 Task 前检查这些依赖；缺失时启动失败并直接显示缺失项以及适用于
Debian/Ubuntu 的安装命令，同时把同一错误写入该次 Task Log。
