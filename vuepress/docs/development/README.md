---
title: 开发
permalink: /development/
createTime: 2026/09/20 00:44:43
---
# 设计理念

Harbor 的目标不是增加一层复杂平台，而是把重复操作沉淀为简单、可复用的任务流程。

## Harbor 是可执行 README

项目的运行方式不应该只存在于某个人的终端历史、聊天记录或一段需要手动复制的
README 中。Harbor 将其中可执行的部分保存为 Task 与 Group：它们与代码一起版本控制，
既是运行配置，也是对项目如何启动的准确说明。

这份说明不是静态文字。GUI、API 和 AI 都使用它启动真实进程，再通过状态与日志验证
结果。文档与运行因此不再是两套容易分离的内容。

## 项目拥有配置

Task 与 Group 保存在项目的 `harbor_taskcfg` 中，而不是 Harbor 的私有数据库中。配置可以随代码提交、审查和复用；Harbor 只负责发现和执行。

## 一个 Task，一个实例

Task 表达一个明确的可执行单元。同一台机器上，一个 Task UUID 同时只能对应一个运行实例。

这条约束让 GUI、Workspace 和 API 看到同一份真实状态，也避免机器人驱动、控制器或服务被意外重复启动。

## 一台机器，一个 Core

每台机器只运行一个 `harbor_core`。Core 统一维护任务进程、UUID、日志和实时状态，不因 Workspace 或连接它的 GUI 而分裂。

GUI 可以关闭，Core 与已经启动的 Task 仍然继续运行。

## Workspace 是视图

Workspace 保存机器连接、搜索路径和日志范围，但不是运行时隔离容器。切换 Workspace 不会停止任务；发现相同 UUID 时，多个 Workspace 共享同一运行状态。

## 本地与远端保持一致

本地和远端都通过同一套 Harbor API 操作 Core。SSH 只负责准备远端 Core，任务发现、运行和日志读取不维护第二套逻辑。

## GUI、AI 与 API 使用同一能力

Harbor GUI 不拥有隐藏的运行能力。界面操作和 Harbor Skill 最终都通过 Core API 完成，因此用户、自动化和 AI 看到的是同一状态模型。

## 严格匹配，不维护兼容层

GUI 与 Core 必须来自同一次 release 构建。Harbor 同时校验版本、API revision 和 Core SHA-256，不为旧 Core 维护兼容分支。

这减少了协议分支和部署歧义，也让错误能够尽早暴露。

## 明确边界

Harbor 是面向单用户和可信局域网的开发工具，不是多租户调度平台、安全网关或通用容器编排系统。功能设计优先考虑清晰、可观察和减少重复劳动。
