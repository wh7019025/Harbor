---
title: 程序接入
permalink: /integration/
createTime: 2026/09/20 00:21:48
---
# 程序接入

Harbor 不要求程序链接专用 SDK。接入的核心是：为程序提供一份 Task YAML，让 `harbor_core` 能以正确的工作目录、环境变量和命令启动它。

```text
你的程序
   ↓ 定义启动方式
harbor_taskcfg/tasks/<task>.yaml
   ↓ Workspace 扫描
harbor_core
   ↓ 启动、停止、记录日志
本地或远端进程
```

## 选择接入方式

- **让 AI Agent 创建或操作任务**：阅读 [Harbor Skill](./agent-skill.md)。
- **普通程序或脚本**：阅读[定义 Task](./task-definition.md)。
- **需要运行参数或环境切换**：阅读[运行时约定](./runtime.md)。
- **提供浏览器控制界面**：阅读[接入 Web Panel](./web-panel.md)。
- **ROS 2 节点或 launch**：阅读[接入 ROS 2](./ros2.md)。

## 最小接入结果

完成接入后，程序应当能够：

1. 被 Harbor 自动发现。
2. 从 GUI 启动、停止和重启。
3. 通过 stdout/stderr 输出可追踪日志。
4. 在本地和远端 Workspace 中使用相同 YAML 运行。
