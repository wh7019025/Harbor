---
title: 使用指南
createTime: 2026/09/19 23:55:58
permalink: /guide/
---
# 什么是 Harbor

Harbor 是一个面向机器人开发的任务编排工具。它把反复输入的启动命令、环境配置和执行顺序，沉淀为项目中可复用的 Task 与 Group。

通过 Harbor，你可以在同一个界面中启动、停止和观察本地或远端机器上的程序，不再依赖一组终端窗口来记住“先运行什么、在哪里运行、出了问题去哪里看”。

![Harbor 实际运行界面：Robot Panel 与 Hello World Loop 正在运行](/images/harbor-app.png)

## Harbor 如何工作

项目通过 `harbor_taskcfg` 保存 YAML 定义，`harbor_core` 负责发现配置、管理进程和记录日志，Harbor GUI 则提供统一的操作入口。

![Harbor GUI、harbor_core、项目配置与任务进程之间的关系](/images/harbor-overview.svg)

配置始终留在项目仓库中，可以随代码一起维护；Workspace 只决定去哪里发现任务，不会复制或接管项目文件。

## Harbor 解决什么问题

- 把重复的启动操作变成可以复用和审查的任务定义。
- 统一管理 ROS 2 节点、机器人服务、无界面程序、WebView 和远端桌面程序。
- 在一个界面中查看运行状态、实时输出与历史日志。
- 用 Group 固化多任务的启动顺序和环境覆盖。
- 在不同 Workspace 中共享同一任务的真实运行状态。

## 建议阅读顺序

1. [关键概念](./concepts.md)：理解 GUI、Core、Workspace、Task 与 Group。
2. [快速开始](./getting-started.md)：安装 Harbor 并运行第一个 Task。
3. [Work with AI](./work-with-ai.md)：让 AI 创建、运行和排查 Harbor 任务。
4. [工作空间](./workspaces.md)：配置扫描路径和远端主机。
5. [远端运行](./remote.md)：了解 SSH、Core 部署和图形环境。
