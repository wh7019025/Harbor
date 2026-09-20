# WebPage View 开发规则

## 不要重复标题

- 页面运行在 Harbor Panel 中，Harbor 标题栏已经展示 Task 或面板名称。
- 页面内容区不要重复应用名、产品名、项目名或 `Dashboard`、`Control Station` 一类无信息量标题。
- 不要添加欢迎语、说明性副标题或纯装饰页头；首屏空间优先用于实时状态、主要操作、告警和数据。
- 分区标题只有在能说明具体内容或操作目的时才保留，例如 `Safety & Motion`、`Joint Feedback`。
- 全局连接状态、运行时间等信息放入已有状态区或工具条，不要为它们额外创建页头。

## Panel 只使用一个视口

- WebPage View 是嵌入式 Panel，不是可上下翻阅的普通网页；根布局必须限制在一个面板视口内。
- Harbor 默认 Panel 窗口为 `1100×780`，这是标准且空间充足的尺寸，不是需要降级的窄屏尺寸。
- 默认尺寸下必须完整呈现主要桌面布局，不能出现整页滚动，也不能因断点设置过大而退化成移动端分页。
- 功能页切换只用于明显小于默认窗口的尺寸，例如接近 `640×480` 的最小窗口。
- 空间不足时使用明确的页签、分段按钮或模式切换；切换时保持状态，不通过整页导航或刷新实现。
- 只允许数据表、日志、Topic 列表等局部数据区域内部滚动；主要操作区不能藏在滚动页面下方。
- 响应式设计的目标是同一时刻完整显示一个功能页，不能把桌面多栏依次堆叠成长页面。

## UI 必须与 Harbor 一致

- 页面必须直接复用 Harbor 的 design tokens、控件样式和交互模式，不能只做颜色近似。
- 新增按钮、输入框、菜单、状态标记等控件前，先检查 Harbor `src/components/` 和 `src/styles/tokens.css` 中的既有实现。
- 禁止在成品界面直接使用浏览器原生 `<select>`；下拉选择统一复用 Harbor `SelectField.vue` 的按钮加弹层方案。
- 不要混用操作系统原生控件、浏览器默认样式和 Harbor 控件；焦点、禁用、悬停、选中和弹层样式必须一致。
- 若示例需要 Harbor 组件尚未支持的能力，应从现有组件扩展最小差异，并保持原 API、tokens 和交互行为。

## 面板端口只有一个配置来源

- 每个 WebPage View 在 `23000–24000` 范围内选择一个项目专用端口，并确认没有与已有 Task 重复。
- 面板端口只能在 Task YAML 的 `webview_interface[].interface_port` 中保存一份。
- 禁止在 YAML `env`、Python、TypeScript、Vite 配置或启动脚本中再次硬编码生产端口。
- Harbor 将单面板声明注入为 `HARBOR_WEBVIEW_NAME`、`HARBOR_WEBVIEW_INTERFACE_PORT` 和 `HARBOR_WEBVIEW_LOCALHOST_ONLY`。
- 面板程序必须读取这些变量；缺少或非法端口时应明确失败，不能静默使用默认端口。
- 多面板 Task 使用 `HARBOR_WEBVIEW_<PANEL_NAME>_*`，其中面板名转为大写并将 `-` 替换为 `_`。
- Vite 开发代理同样读取 `HARBOR_WEBVIEW_INTERFACE_PORT`，未提供时不能偷偷代理到硬编码端口。
