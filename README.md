# Harbor

Harbor 桌面控制台：启动即 TaskClick 主界面；Setting / AgentHelp 在顶栏弹层内。

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

## 打包

```bash
npm run tauri build -- --bundles deb
```

产物：`src-tauri/target/release/bundle/deb/Harbor_*_amd64.deb`

## CI

GitHub Actions（`.github/workflows/build.yml`）会在 `main` / PR / 手动触发时构建 Linux `.deb`，并上传为 workflow artifact。

推送 `v*` tag（例如 `v0.1.0`）时，会额外创建并公开 Release，挂上 `.deb`。

## 数据

- Task / Group：默认 `~/.harbor/st_taskcfg/{tasks,groups,log}`
- 项目内：`<项目>/st_taskcfg/{tasks,groups}`
- 设置：`~/.harbor/settings.json`（含 `search_paths`；Agent 可直接编辑，见 `agent_doc/settings.md`）
