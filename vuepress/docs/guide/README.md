---
title: 使用指南
createTime: 2026/09/19 23:55:58
permalink: /guide/
---
# 什么是 Harbor

**Harbor 是项目的可执行 README。**

传统 README 告诉你需要进入哪个目录、加载什么环境、依次运行哪些命令；Harbor 把这些
运行说明沉淀为项目中的 Task 与 Group，让它们可以被直接执行、观察和复用。

它仍然是一套面向机器人开发的简洁任务编排工具，但项目不再依赖某个人记住“先运行
什么、在哪里运行、出了问题去哪里看”。人、GUI 和 AI 看到的是同一份运行定义。

![Harbor 实际运行界面：Robot Panel 与 Hello World Loop 正在运行](/images/harbor-app.png)

## Harbor 如何工作

项目通过 `harbor_taskcfg` 保存 YAML 定义，`harbor_core` 负责发现配置、管理进程和记录日志，Harbor GUI 则提供统一的操作入口。

![Harbor GUI、harbor_core、项目配置与任务进程之间的关系](/images/harbor-overview.svg)

配置始终留在项目仓库中，可以随代码一起维护；Workspace 只决定去哪里发现任务，不会复制或接管项目文件。

## Harbor 解决什么问题

Harbor 解决的不是“如何运行一条命令”，而是如何让项目的运行方式像代码一样留下来。

- 把 README 中的命令、环境和启动顺序沉淀为可执行的 Task 与 Group。
- 在同一个入口完成启动、停止、状态观察和日志追踪。
- 让团队成员与 AI 复用同一套流程，而不是各自维护一组终端命令。

## 建议阅读顺序

1. [关键概念](./concepts.md)：理解 GUI、Core、Workspace、Task 与 Group。
2. [快速开始](./getting-started.md)：安装 Harbor 并运行第一个 Task。
3. [Work with AI](./work-with-ai.md)：让 AI 创建、运行和排查 Harbor 任务。
4. [工作空间](./workspaces.md)：配置扫描路径和远端主机。
5. [远端运行](./remote.md)：了解 SSH、Core 部署和图形环境。
