---
title: Group YAML
permalink: /reference/group-yaml/
createTime: 2026/09/20 00:44:43
---
# Group YAML

```yaml
version: "0.2.0-preview.2"
uuid: 4c9e2d73-6b15-4f80-a271-95d38c7e1a42
id: system-info
name: System Info
description: 收集基础系统信息
tasks:
  - task: uc-info
    wait_after_sec: 0
    env: {}
  - task: demo-ping
    config: development
    wait_after_sec: 1.5
    env:
      MODE: quick
```

## 字段

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `version` | 是 | Harbor 应用版本。 |
| `uuid` | 自动 | Group 稳定身份；缺失时自动生成。 |
| `id` | 是 | 同一 `groups/` 目录内唯一。 |
| `name` | 否 | 界面显示名。 |
| `description` | 否 | 编排目的说明。 |
| `tasks` | 是 | 按顺序执行的 Task 列表。 |
| `tasks[].task` | 是 | Task 短 id。 |
| `tasks[].config` | 否 | 指定 Task config。 |
| `tasks[].wait_after_sec` | 否 | 启动当前 Task 后的等待秒数。 |
| `tasks[].env` | 否 | 此次执行追加或覆盖的环境变量。 |

旧配置中的 `tasks[].prefix_path` 仍可读取，新配置不应继续写入。
