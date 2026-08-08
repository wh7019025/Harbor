# Task / Group 配置目录

- 全局配置根目录：`~/.harbor/harbor_taskcfg`（可由 `settings.json` 的 `taskcard_root` 修改）
  - 任务：`~/.harbor/harbor_taskcfg/tasks/**/*.yaml`
  - 组：`~/.harbor/harbor_taskcfg/groups/**/*.yaml`
- 项目配置：优先放在项目根的 `harbor_taskcfg/` 中
  - 任务：`{project}/harbor_taskcfg/tasks/**/*.yaml`
  - 组：`{project}/harbor_taskcfg/groups/**/*.yaml`
- 项目必须位于 `~/.harbor/settings.json` 的 `search_paths` 某个条目下。Harbor 从每个条目向下最多 **5 层**查找 `harbor_taskcfg/tasks/` 和 `harbor_taskcfg/groups/`。

## 选择位置

- 与仓库代码一起维护、需要团队共享：写入项目的 `harbor_taskcfg/`。
- 仅适用于当前机器或跨项目复用：写入全局配置根目录。
- 目录不存在时可直接创建 `tasks/` 或 `groups/`。

## 配置身份

- 运行时身份为 `(prefix_path, id)`，YAML 仍只写短 `id`。
- 全局配置的 `prefix_path` 是 `taskcard_root` 的绝对路径。
- 项目配置的 `prefix_path` 是该项目下 `harbor_taskcfg` 的父目录。
- 跨工程可存在同名 id；同一 `tasks/` 或 `groups/` 源目录内的 id 必须唯一。

Agent 不应在 YAML 中写顶层 `prefix_path`；它由 Harbor 根据文件位置生成。Group 的任务条目在消除跨项目同名歧义时可以写 `prefix_path`。
