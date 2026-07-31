# 给 Agent 的建议

1. 改仓库内任务时，优先写到项目下的 `st_taskcfg/tasks/` / `st_taskcfg/groups/`，并把**项目根**加入 `search_paths`（格式见 [settings.md](settings.md)）
2. 跨工程可复制同名短 `id`；启停与编辑按 `(prefix_path, id)` 定位。Group 条目可用可选 `prefix_path` 指定工程，未写则取第一个命中
3. 改完 YAML 后让用户在 TaskClick 点 research，或重启窗口
4. 不要手改正在 running 的任务定义文件；先 stop 再 edit
5. 配置与知识目录：`~/.harbor/`（设置：[settings.json](settings.md)；任务配置：`st_taskcfg/`；文档：`agent_doc/`；MCP：`mcp/`）
6. Harbor 每次启动会刷新 `~/.harbor/agent_doc/`
7. 可选：配置 Cursor MCP，读取 `harbor://agent_doc/*` resources（见 [MCP.md](MCP.md)）
