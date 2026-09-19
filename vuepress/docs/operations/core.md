---
title: core
createTime: 2026/09/19 23:57:11
permalink: /operations/am55k4lj/
---
# harbor_core

## 单机单实例

一台机器只允许运行一个 `harbor_core`，无论连接它的 GUI、Harbor 版本或 Workspace 是什么。这样才能保证任务状态和进程所有权唯一。

如果 GUI 发现正在运行的 Core 与自身不匹配，会先关闭旧 Core，再启动配套版本。多个 GUI 同时争用同一机器会导致 Core 被反复替换，因此当前设计只支持单用户独占访问。

## 管理目录

GUI 从 `~/.harbor/core/<version>/harbor_core` 使用受管理的 release 二进制。构建时生成的 SHA-256 固化在 GUI 中，连接时同时检查：

1. Harbor 版本。
2. Core API revision。
3. Core 二进制 SHA-256。

任一项不匹配都不会复用该 Core。

## 状态含义

- **已连接 · localhost · 正常**：本机 Core 可访问且匹配。
- **已连接 · 10.x.x.x · 正常**：远端 Core 可访问且匹配。
- **版本不符**：Core 版本、API revision 或 hash 不匹配。
- **关闭**：地址可配置，但没有可用 Core。
- **不可达**：网络、端口或进程状态阻止访问。

## Probe、Redeploy 与 Force deploy

- **Probe**：只探测 Core 与系统状态，不复制、不重启。
- **Redeploy**：发现缺失或不匹配时重新部署匹配 Core。
- **Force deploy**：忽略当前检测结果，强制覆盖并重启 Core；仅用于文件损坏或自动检测无法恢复的情况。
