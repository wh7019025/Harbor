---
title: task-yaml
createTime: 2026/09/19 23:56:32
permalink: /reference/3trlj7r1/
---
# Task YAML

## 完整示例

```yaml
version: "0.2.0-preview"
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
panel_interface:
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
| `panel_interface` | 否 | Web 面板声明列表。 |
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
