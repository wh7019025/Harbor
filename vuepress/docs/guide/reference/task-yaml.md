---
title: Task YAML
permalink: /reference/task-yaml/
createTime: 2026/09/20 00:44:43
---
# Task YAML

## 完整示例

```yaml
version: "0.2.0-preview.2"
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
| `vnc_interface` | 否 | 远端桌面程序的 VNC 页面，最多一个。 |
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

## 桌面程序可视化

Qt、GTK 等桌面程序无需改造成 Web 应用，只需声明 VNC 接口：

```yaml
vnc_interface:
  - panel_name: desktop
    interface_port: 23682
command:
  argv: [your-gui-program]
```

local workspace 仍然直接打开原生窗口，不启动 VNC，也不显示该 Panel。只有在
remote workspace 中，Harbor 才自动创建虚拟 X11、TigerVNC 和 noVNC 环境，
并对当前局域网提供桌面入口。一个 Task 最多只能配置一个 `vnc_interface`。

不要在这类 Task 的 `env` 或启动脚本中手写 `DISPLAY`、`XAUTHORITY`、
`WAYLAND_DISPLAY`、`QT_QPA_PLATFORM` 或 `GDK_BACKEND`。远端显示环境由 Harbor
管理，本地则继承当前图形会话；固定 `DISPLAY=:0` 会绕过 VNC。

其中 `interface_port` 是 noVNC 页面使用的 HTTP/WebSocket 端口，不是 VNC TCP
端口；TigerVNC 本身只使用临时 Unix Socket。

远端机器需要安装 `tigervnc-standalone-server`、`novnc`、`websockify` 和 `openbox`。
Harbor 会在启动 Task 前检查这些依赖；缺失时启动失败并直接显示缺失项以及适用于
Debian/Ubuntu 的安装命令，同时把同一错误写入该次 Task Log。
