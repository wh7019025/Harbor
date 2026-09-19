# 日志

- 运行中：TaskClick 右侧 live 流式输出
- 停止后：自动切到 Monaco 历史查看（完整日志，超过 1MiB 显示末尾）
- 文件名：`{task_id}-{YYMM-DDHHMMSS}.log`（本地时间，如 `demo-ping-2507-28224430.log`）
  写入 core 所在机器的 `~/.harbor/workspace/<id>/log/`，各 workspace 独立显示和保留
- 仅保留最近 **50** 条（按时间）；超出的历史文件会删除，正在跑的 live 日志不会删

Harbor GUI 与 harbor_core 自身日志写在各自机器的 `~/.harbor/log/harbor.log`。Logs 面板默认显示这份按时间聚合的 Harbor log；点 Harbor 按钮可关掉，再点左侧任务 log 可切换。
