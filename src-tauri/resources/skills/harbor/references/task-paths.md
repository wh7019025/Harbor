# Task / Group 配置目录

- 项目配置：放在项目根的 `harbor_taskcfg/` 中
  - 任务：`{project}/harbor_taskcfg/tasks/**/*.yaml`
  - 组：`{project}/harbor_taskcfg/groups/**/*.yaml`
- 项目必须位于**当前 workspace** 的 `search_paths` 某个条目下。Harbor 从每个条目向下最多 **5 层**查找 `harbor_taskcfg/tasks/` 和 `harbor_taskcfg/groups/`。
- local workspace 的路径是 harbor_core 本机目录；remote workspace 的路径是远端 harbor_core 所在机器的目录。
- 日志在 **core 所在机器** 的 `~/.harbor/workspace/<id>/log/`，按 workspace 隔离；全机进程占用记录在 `~/.harbor/runtime/run/`，按 UUID 跨 workspace 共享。这些目录都不存放 Task / Group YAML。

## 选择位置

- 与仓库代码一起维护：写入项目的 `harbor_taskcfg/`。
- 目录不存在时可直接创建 `tasks/` 或 `groups/`。
- 没有 search path 时不能创建 Task / Group；先把项目根加入当前 workspace 的 `search_paths`。

## 配置身份

- 运行时身份为 YAML 顶层 `uuid`；同一台机器、同一 UUID 同时只能运行一个实例。
- 项目配置的 `prefix_path` 是该项目下 `harbor_taskcfg` 的父目录。
- 跨工程可存在同名 id；同一 `tasks/` 或 `groups/` 源目录内的 id 必须唯一。
- `uuid` 缺失时 harbor_core 会自动生成并写回 YAML；发现重复 UUID 时会拒绝启动，并要求为其中一个文件重置 UUID。

workspace 只控制配置发现范围和远端 core 目标。切换 workspace 不会停止任务；再次发现同一 UUID 时会共享其实时运行状态。Agent 不应在 YAML 中写顶层 `prefix_path`；它由 Harbor 根据文件位置生成。Group 使用 Task `id` 引用任务，并按“同目录优先、跨目录唯一匹配”的规则解析。
