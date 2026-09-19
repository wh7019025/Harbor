---
title: remote
createTime: 2026/09/19 23:55:59
permalink: /guide/mwitrc3g/
---
# 远端运行

![Harbor 远端连接流程](/images/remote-flow.svg)

## 连接流程

远端 Workspace 的连接分为四步：

1. 通过 SSH 探测远端系统与已有 Core。
2. 比较 GUI 固化的 Core SHA-256、版本和 API revision。
3. 不匹配时部署当前 GUI 配套的 release Core。
4. 启动远端 Core，并通过 `http://<host>:29385` 获取状态。

正常切换不应重复复制 Core；只有缺失、校验不匹配或显式强制部署时才复制。

## 图形程序

Core 会尝试继承远端用户图形会话中的 `DISPLAY`、`XAUTHORITY`、Wayland 与 DBus 环境。远端机器仍必须有可用的桌面会话，例如物理登录或 VNC 会话。

如果没有图形会话，Qt 程序可能报告：

```text
qt.qpa.xcb: could not connect to display
```

这不是 Harbor 能虚拟出的显示设备。无界面任务可改用 offscreen 模式；必须显示窗口时应配置 VNC、RDP 或其他远端桌面。

## 打开远端路径

本机 `xdg-open` 不能直接打开远端 Linux 路径。可使用支持 SFTP 的文件管理器，或在编辑器中配置 Remote SSH。Harbor 的远端路径操作应被视为导航入口，而不是远端文件系统挂载。

## 网络要求

- SSH 端口从 GUI 所在机器可达。
- TCP `29385` 从 GUI 所在机器可达。
- Web 面板端口按需开放。
- 不应把 Core 端口暴露到不可信网络。
