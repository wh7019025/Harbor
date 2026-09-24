---
title: 构建 Harbor
permalink: /development/build/
createTime: 2026/09/20 00:44:43
---
# 构建 Harbor

## Git 版本

Harbor 的有效版本只由 Git 管理。发布提交使用精确标签，例如：

```bash
git tag -a v0.2.0-preview.6 -m "Harbor 0.2.0 preview.6"
```

查看当前构建将使用的版本：

```bash
scripts/git_version.sh
```

位于 Tag 上的干净提交输出 `0.2.0-preview.6`。Tag 之后的开发提交输出类似
`0.2.0-preview.6+3.g4d65765`；有未提交改动时再附加 `.dirty`。

`package.json`、Cargo manifest 与 `tauri.conf.json` 中统一使用 `0.0.0` 占位，不再保存或人工同步应用版本。源码包脱离 Git 时，构建脚本回退到基线版本 `0.2.0-preview.6`。

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

该命令构建 release `harbor_core`，同时固化构建机上的 `ttyd`，并为两个二进制生成
SHA-256。GUI 构建时读取并固化这些哈希，用于本机 Core 管理和远端终端自动部署。

构建脚本会下载并校验固定版本的官方静态 `ttyd`，也可以通过 `HARBOR_TTYD_BIN` 指定
另一份静态二进制。最终 Debian 包会携带这份 `ttyd`；构建机和使用 Harbor 的远端机器
都不需要通过系统包管理器安装它。

远端部署和 `~/.harbor/core/` 都必须使用 release Core，debug 产物不能替代。托管
`ttyd` 同样按哈希存放，并与 GUI 构建保持一致。

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

普通构建把安装包上传为 workflow artifact。推送 `v*` tag 时，GitHub Actions 还会创建对应 Release，附加 `.deb` 安装包，并根据上一 Tag 自动生成版本变更说明。

Release workflow 要求当前提交存在精确的 `v*` Tag，并拒绝带未提交改动的发布构建。Rust GUI、`harbor_core`、Tauri Bundle 与 `.deb` 文件名使用同一个 Git 版本。

文档由 `.github/workflows/docs.yml` 独立构建并部署到 GitHub Pages。修改 `vuepress/**`、根 `package.json` 或 lockfile 时会触发文档 workflow。
