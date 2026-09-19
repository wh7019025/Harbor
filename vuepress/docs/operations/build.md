---
title: 构建 Harbor
permalink: /operations/build/
createTime: 2026/09/20 00:21:16
---
# 构建 Harbor

## 前端检查

```bash
npm install
npm run build
```

`npm run build` 执行 TypeScript 类型检查和 Vite 生产构建，但不会生成桌面安装包。

## 开发模式

```bash
npm run tauri:dev
```

开发模式启动 Vite 与 Tauri 窗口，适合界面和 Core 联调。

## release Core

```bash
npm run core:release
```

该命令构建 release `harbor_core` 并更新 GUI 使用的 hash 信息。远端部署和 `~/.harbor/core/` 都必须使用 release 产物。

## 桌面安装包

```bash
npm run tauri:build
```

当前脚本生成 Debian 安装包。发布前至少执行前端构建、Rust 测试和一次本地连接验证。

## 文档站

```bash
npm install
npm run docs:dev
npm run docs:build
```

应用与文档由 Harbor 根目录的 npm workspace 统一管理，不需要进入 `vuepress/` 单独安装依赖。
