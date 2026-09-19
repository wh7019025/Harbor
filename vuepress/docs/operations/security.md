---
title: security
createTime: 2026/09/19 23:57:12
permalink: /operations/dls9ediz/
---
# 安全

## “无鉴权”是什么意思

Core API 当前不要求密码、Token 或客户端证书。任何能访问 Core 端口的设备都可能读取任务信息、启动或停止任务、修改 YAML 和读取日志。

## 安全部署边界

- 本地 Core 默认只监听 `127.0.0.1`。
- 远端 Core 只应位于可信局域网、VPN 或严格防火墙之后。
- 禁止把 TCP `29385` 直接映射到公网。
- 不要在 Task YAML 中保存密码、Token 或 SSH 私钥。
- 任务需要密钥时，优先使用系统密钥存储或运行环境注入。

## sudo

`sudo: true` 会提高任务权限，也会扩大错误命令的影响范围。只为确实需要系统权限的任务启用，并限制可执行内容。

## Web 面板

Harbor 不会自动为 Task 自己提供的 Web 面板增加鉴权。面板若监听非 localhost 地址，应自行实现访问控制，或仅在受控网络中使用。
