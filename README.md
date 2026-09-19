# Harbor

[![Build](https://github.com/wh7019025/Harbor/actions/workflows/build.yml/badge.svg)](https://github.com/wh7019025/Harbor/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/wh7019025/Harbor)](https://github.com/wh7019025/Harbor/releases)
[![Platform](https://img.shields.io/badge/platform-Linux-blue)](https://github.com/wh7019025/Harbor)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3-42b883?logo=vue.js&logoColor=white)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust&logoColor=white)](https://www.rust-lang.org)

Harbor 是一个面向开发者与 AI Agent 的本地任务管理工具，用统一配置组织、编排和运行工作流。

## 系统依赖（Linux）

开发前请先安装：

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

装完后确认：

```bash
pkg-config --exists webkit2gtk-4.1 && echo webkit_ok
pkg-config --exists gtk+-3.0 && echo gtk_ok
```

## 开发

```bash
npm install
npm run tauri dev
```

`npm run tauri dev` 会先执行 `npm run core:release`，生成 release `harbor_core` 和相邻的 `harbor_core.sha256`。GUI 编译时校验并固化该哈希。

## 打包

```bash
npm run tauri build -- --bundles deb
```

产物：`src-tauri/target/release/bundle/deb/Harbor_*_amd64.deb`（含 `harbor` 与 `harbor_core`）

## CI

GitHub Actions（`.github/workflows/build.yml`）会在 `main` / PR / 手动触发时构建 Linux `.deb`，并上传为 workflow artifact。

推送 `v*` tag（例如 `v0.1.2-rc3`）时，会额外创建并公开 Release，挂上 `.deb`。

## 数据

- Task / Group：项目内 `<项目>/harbor_taskcfg/{tasks,groups}`
- 日志：core 所在机器的 `~/.harbor/workspace/<id>/log`，各 workspace 独立
- 进程占用状态：core 所在机器的 `~/.harbor/runtime/run`，按 UUID 跨 workspace 共享
- Harbor 自身日志：`~/.harbor/log/harbor.log`（GUI 与 core 聚合显示）
- 设置：`~/.harbor/settings.json`（含当前 workspace 的 `search_paths`；Agent 可直接编辑，见 `agent_doc/settings.md`）
- 本机 daemon：GUI 只执行 `~/.harbor/core/<version>/harbor_core` 中的托管副本（默认 `http://127.0.0.1:29385`）。该副本必须与 GUI 内固化的 release core SHA-256 一致；打包目录或 `/usr/bin/harbor_core` 仅作为安装来源，不能直接运行。GUI 关闭后 core 仍可运行。每台机器同一时间只运行一个 core，不区分版本；当前 GUI 访问时会自动关闭版本、API 或哈希不对应的旧 core 并启动匹配 core。
- Task / Group YAML 都有全局 `uuid`。缺失时 core 自动写入；同一台机器发现重复 UUID 时禁止启动，需为其中一个配置重置 UUID。workspace 只决定当前发现哪些配置，不隔离运行实例。
