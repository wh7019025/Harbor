# Group YAML

```yaml
version: "0.1.2-rc1"
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
| `tasks[].task` | 是 | 已存在的 Task `id` |
| `tasks[].wait_after_sec` | 否 | 当前 Task 后等待的秒数，默认 `0` |
| `tasks[].env` | 否 | 对此次组内执行追加或覆盖的环境变量，默认 `{}` |

## Task 引用规则

Group 只需用 `tasks[].task` 引用 Task 短 id，不要写项目绝对路径。

Harbor 在保存和执行 Group 前按以下顺序检查：

1. 优先查找与 Group 位于同一个 `harbor_taskcfg` 的同 id Task。
2. 同目录没有时，搜索所有已发现的 Task。
3. 只有一个结果时使用该 Task。
4. 有多个结果时报告所有候选位置并拒绝保存或执行，不按加载顺序猜测。
5. 完整 Group 的所有引用检查通过后，才开始执行第一个 Task。

旧配置中的 `tasks[].prefix_path` 仍可读取，但新建或维护 Group 时不应再写入。

顶层 `folder`、`prefix_path` 由 Harbor 自动生成，不要写入 YAML。
