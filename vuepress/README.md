# Harbor Documentation

Harbor 官方仓库中的产品与开发文档，基于 VuePress 2 和 VuePress Theme Plume 构建。

## 本地开发

在 Harbor 仓库根目录执行：

```bash
npm install
npm run docs:dev
```

生产构建：

```bash
npm run docs:build
```

所有依赖统一记录在仓库根目录的 `package-lock.json`。文档入口位于 `vuepress/docs/README.md`，导航与侧边栏配置位于 `vuepress/docs/.vuepress/`。
