---
title: 关键概念
createTime: 2026/09/19 23:55:58
permalink: /guide/pmxfl5gq/
---
# 关键概念

![Harbor 系统总览](/images/harbor-overview.svg)

## Harbor GUI

桌面界面负责配置工作空间、展示任务、打开日志和面板。GUI 不直接托管任务进程，而是通过 HTTP 与 `harbor_core` 通信。

## harbor_core

Core 负责发现 YAML、启动进程、记录日志和维护实时状态。一台机器只允许一个 Core 实例，不区分由哪个 GUI 或 Workspace 启动。

## Workspace

Workspace 是一组独立的用户视图，保存自己的搜索路径和日志目录。它可以连接本机，也可以通过 SSH 指向远端机器。

切换 Workspace 不会停止任务。任务的运行身份由 UUID 决定，因此另一 Workspace 发现相同 UUID 时会看到相同状态。

## Task

Task 描述一个可执行单元，包括工作目录、环境变量、命令、可选运行配置和 Web 面板。每个 Task 同时最多运行一个实例。

## Group

Group 按顺序启动多个 Task，可为每个条目选择 config、覆盖环境变量并设置启动后的等待时间。

## UUID

`uuid` 是 Task 与 Group 的稳定身份，`id` 主要用于人类阅读和同目录引用。

- YAML 缺少 UUID 时，Core 自动生成并写回。
- 两份 YAML 使用相同 UUID 时，Core 报告冲突。
- 用户应为其中一份配置重置 UUID，而不是让系统猜测。
