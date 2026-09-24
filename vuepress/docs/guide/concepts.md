---
title: 关键概念
createTime: 2026/09/19 23:55:58
permalink: /guide/concepts/
---
# 关键概念

![Harbor 系统总览](/images/harbor-overview.svg)

## 可执行 README

`harbor_taskcfg` 是项目运行方式的可执行说明。它和源码放在一起，回答三件事：程序如何
启动、多个程序如何配合、运行后去哪里观察。

普通 README 仍适合解释背景、安装依赖和设计决策；Harbor 接管其中需要被反复执行的
部分。Task 描述一个可运行程序，Group 描述一套可运行流程，日志和状态则告诉你这份
说明是否真的运行成功。

## Harbor GUI

桌面界面负责配置工作空间、展示任务、打开日志、WebView 和 VNC 页面。GUI 不直接托管任务进程，而是通过 HTTP 与 `harbor_core` 通信。

## harbor_core

Core 负责发现 YAML、启动进程、记录日志和维护实时状态，也统一托管远端终端、虚拟
VNC 与真实桌面抓取服务。一台机器只允许一个 Core 实例，不区分由哪个 GUI 或 Workspace
启动。

## Workspace

Workspace 是保存在 Harbor GUI 本机的一组机器连接与项目视图，保存自己的搜索路径。
它可以连接本机，也可以通过 SSH 指向远端机器。

本地 Workspace 的任务日志按 Workspace 分目录保存；远端任务日志属于远端机器上的
Task 运行记录，不因本地 Workspace 改名或切换而迁移。

切换 Workspace 不会停止任务。任务的运行身份由 UUID 决定，因此另一 Workspace 发现相同 UUID 时会看到相同状态。

## Task

Task 是可执行 README 中的一条运行说明，包含工作目录、环境变量、命令、可选运行配置
以及 WebView 或 VNC 界面。每个 Task 同时最多运行一个实例。

## Group

Group 是可执行 README 中的一套完整流程。它按顺序启动多个 Task，可为每个条目选择
config、覆盖环境变量并设置启动后的等待时间。
