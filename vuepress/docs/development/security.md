---
title: 安全说明
permalink: /development/security/
createTime: 2026/09/20 00:44:43
---
# 安全说明

Harbor 设计用于**单用户和可信局域网内部**的机器人开发环境。

Harbor Core API 和 Web Panel 不提供默认鉴权，也不作为安全边界。任何能够访问相应端口的设备，都可能读取状态、修改配置或起停任务。

::: danger
Harbor 不提供安全保证。不要将 Harbor Core 或未经保护的 Web Panel 暴露到公网或不可信网络。
:::

需要跨网络使用时，由部署者通过可信 VPN、防火墙和网络隔离自行保护访问范围。
