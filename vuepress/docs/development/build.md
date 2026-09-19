---
title: 构建 Harbor
permalink: /development/build/
createTime: 2026/09/20 00:44:43
---
# 构建 Harbor

## 1. 前端检查

```bash
npm install
npm run build
```

`npm run build` 执行 TypeScript 类型检查和 Vite 生产构建，但不会生成桌面安装包。

## 2. Rust 测试

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

先验证 Core、配置解析和 API 行为，再生成发布产物。

## 3. release Core

```bash
npm run core:release
```

该命令构建 release `harbor_core`，并生成相邻的 `harbor_core.sha256`。GUI 构建时读取并固化这个哈希。

远端部署和 `~/.harbor/core/` 都必须使用 release Core，debug 产物不能替代。

## 4. 开发模式

```bash
npm run tauri:dev
```

开发模式启动 Vite 与 Tauri 窗口，适合界面和 Core 联调。

## 5. 桌面安装包

```bash
npm run tauri:build
```

该命令会自动执行 release Core 和前端构建，生成 Debian 安装包：

```text
src-tauri/target/release/bundle/deb/Harbor_*_amd64.deb
```

发布前至少执行前端构建、Rust 测试和一次本地连接验证。

## 6. 文档站

```bash
npm install
npm run docs:dev
npm run docs:build
```

应用与文档由 Harbor 根目录的 npm workspace 统一管理，不需要进入 `vuepress/` 单独安装依赖。

## 7. CI 与 Release

`.github/workflows/build.yml` 在以下情况构建 Linux `.deb`：

- 推送到 `main`。
- 创建或更新 Pull Request。
- 手动触发 workflow。

普通构建把安装包上传为 workflow artifact。推送 `v*` tag 时，GitHub Actions 还会创建对应 Release，并附加 `.deb` 安装包。

文档由 `.github/workflows/docs.yml` 独立构建并部署到 GitHub Pages。修改 `vuepress/**`、根 `package.json` 或 lockfile 时会触发文档 workflow。
