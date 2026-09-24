---
title: 安全说明
permalink: /development/security/
createTime: 2026/09/20 00:44:43
---
# 安全说明

Harbor 设计用于**单用户和可信局域网内部**的机器人开发环境。

Harbor Core API、WebView、noVNC 和 ttyd 不提供自己的登录鉴权，也不作为安全边界。
Core API 与选择监听远端网卡的 WebView 只能用于可信网络；Harbor 托管的 noVNC 和 ttyd
固定监听远端回环地址，由 GUI 通过已有 SSH 凭据建立 Tunnel。

::: danger
Harbor 不提供安全保证。不要将 Harbor Core、WebView 或任何 Harbor 内部服务暴露到
公网或不可信网络。
:::

需要跨网络使用时，由部署者通过可信 VPN、防火墙和网络隔离自行保护访问范围。
