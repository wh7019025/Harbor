---
title: 环境配置
permalink: /development/environment/
createTime: 2026/09/20 01:52:37
---
# 环境配置

Harbor 当前面向 Linux 开发，桌面端使用 Vue 3、Vite、Tauri 2 和 Rust。

## 基础工具

安装以下工具：

- Node.js LTS 与 npm。
- Rust stable 与 Cargo。
- Git。
- Debian/Ubuntu 系统构建依赖。

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  pkg-config \
  build-essential
```

确认 WebKitGTK 与 GTK 可被构建系统发现：

```bash
pkg-config --exists webkit2gtk-4.1 && echo webkit_ok
pkg-config --exists gtk+-3.0 && echo gtk_ok
```

## 安装项目依赖

在 Harbor 仓库根目录执行：

```bash
npm install
```

根目录的 npm workspace 同时管理 Harbor GUI 与 VuePress 文档，不需要进入 `vuepress/` 单独安装。

## 启动开发模式

```bash
npm run tauri:dev
```

启动流程会先构建 release `harbor_core` 并生成 SHA-256，再启动 Vite 和 Tauri GUI。Core 必须使用 release 构建，即使 GUI 当前运行的是 debug 版本。

前端开发服务器使用 `http://localhost:1420`。如果启动时报端口已占用，应先关闭旧的 Vite 或 Harbor 开发进程。

## 只开发前端

```bash
npm run dev
```

该命令只启动 Vite，不启动 Tauri 窗口或 Core 管理逻辑，适合处理纯页面样式；需要验证 Workspace、Task、远端连接或桌面窗口时，应使用 `npm run tauri:dev`。

## 开发文档

```bash
npm run docs:dev
```

文档生产构建使用：

```bash
npm run docs:build
```
