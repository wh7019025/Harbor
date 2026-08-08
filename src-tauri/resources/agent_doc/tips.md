# 配置维护检查

1. 仓库相关配置优先写入项目的 `harbor_taskcfg/tasks/` 和 `harbor_taskcfg/groups/`。
2. 跨工程可复用同名短 `id`，但定位时使用 `(prefix_path, id)`。
3. 新增或修改 YAML 后，告知用户重新加载配置或重启 Harbor；无需指导界面操作。
4. 不要修改正在运行的 Task 定义；应先停止任务。
5. 修改已有文件时保留用户未要求变更的字段、命令和环境变量。
6. 创建 Group 前确认每个 Task 已存在；同名 Task 必须用 `prefix_path` 明确指向。
7. 检查 `version`、`id`、`workdir`、命令二选一规则，并确认 YAML 可解析。
