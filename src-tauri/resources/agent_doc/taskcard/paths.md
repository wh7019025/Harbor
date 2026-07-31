# TaskCard 目录约定

- 默认根目录：`~/.harbor/st_taskcfg`（可用 Setting 改 `taskcard_root`）
  - 任务：`~/.harbor/st_taskcfg/tasks/**/*.yaml`
  - 组：`~/.harbor/st_taskcfg/groups/**/*.yaml`
  - 日志：`~/.harbor/st_taskcfg/log/`
- Search path：在 `~/.harbor/settings.json` 的 `search_paths` 中配置（见 [settings.md](../settings.md)），或在 TaskClick 的 paths panel / Setting 弹层添加。Harbor 会向下最多 **5 层** 查找：
  - `st_taskcfg/tasks/` → 任务 YAML
  - `st_taskcfg/groups/` → 组 YAML
- 分类名：默认用搜索根目录的最后一段（如 `/home/se/Harbor` → `Harbor`）
- Research：paths panel 内可重新扫描（修改 `search_paths` 后需先重启 Harbor 或 Setting save，再 research）
- 运行时身份 `(prefix_path, id)`（YAML 仍写短 `id`；跨工程可同名）：
  - Root：`prefix_path` = `taskcard_root` 绝对路径（如 `/home/se/.harbor/st_taskcfg`）
  - 项目：`prefix_path` = 该项目下 `st_taskcfg` 的父目录（如 `/home/se/Harbor`）
  - 同一 `tasks/` 或 `groups/` 源目录内短 id 仍唯一
