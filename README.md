# SuperTerm

桌面控制台：Launcher 打开 SystemPanel / TaskClick / Setting。

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

产物：`src-tauri/target/release/bundle/deb/SuperTerm_*_amd64.deb`

## 数据

- Task / Group：默认 `~/.superterm/st_taskcfg/{tasks,groups,log}`
- 项目内：`<项目>/st_taskcfg/{tasks,groups}`
- 设置：`~/.superterm/settings.json`
