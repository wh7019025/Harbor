# Group YAML

```yaml
version: "0.1.1"
id: system-info
name: System Info
description: Collect basic system information
tasks:
  - task: uc-info
    wait_after_sec: 0
    env: {}
  - task: demo-ping
    wait_after_sec: 0
    env:
      MODE: quick
```

## 字段

| 字段 | 必填 | 说明 |
|------|------|------|
| `version` | 是 | `harbor --version` 的输出；Agent 直接编辑时须手动更新 |
| `id` | 是 | 字母数字、`-`、`_`；同一 `groups/` 目录内唯一 |
| `name` | 否 | 显示名 |
| `description` | 否 | 组说明 |
| `tasks` | 是 | 按顺序执行的 Task 条目列表 |
| `tasks[].task` | 是 | 已存在的 Task 短 id |
| `tasks[].wait_after_sec` | 否 | 当前 Task 后等待的秒数，默认 `0` |
| `tasks[].env` | 否 | 对此次组内执行追加或覆盖的环境变量，默认 `{}` |
| `tasks[].prefix_path` | 否 | Task 所属项目的绝对路径，用于消除跨项目同名歧义 |

## 引用规则

- Task id 在所有已加载配置中唯一时，可以只写 `task`。
- 存在同名 Task 时应写 `prefix_path`，不要依赖加载顺序。
- 跨工程允许同名 Group；同一 `groups/` 源目录内短 id 仍须唯一。
- 顶层 `folder`、`prefix_path` 由 Harbor 自动生成，不要写入 YAML。
