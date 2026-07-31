# Harbor 设置（settings.json）

配置文件路径：`~/.harbor/settings.json`

Agent **可以直接编辑此文件** 修改 Search paths、taskcard 根目录与指标轮询间隔。格式为 JSON。

## 完整格式

```json
{
  "taskcard_root": "~/.harbor/st_taskcfg",
  "search_paths": [
    "/home/seen-e-embodied/wh_workspace",
    "/home/se/Harbor"
  ],
  "metrics_fast_ms": 1000,
  "metrics_slow_ms": 10000
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `taskcard_root` | string | 默认 Task/Group 根目录（Root 分类）。可用 `~/...` 或绝对路径 |
| `search_paths` | string[] | 额外搜索的项目根目录，每项一行路径；见下文 |
| `metrics_fast_ms` | number | CPU / 网络 / GPU 等指标刷新间隔（毫秒，≥ 200） |
| `metrics_slow_ms` | number | 内存 / 磁盘等慢指标刷新间隔（毫秒，≥ 1000） |

文件不存在时，Harbor 使用内置默认值（空 `search_paths`，`taskcard_root` 为 `~/.harbor/st_taskcfg`）。

## Search paths

`search_paths` 中的每一项必须是**已存在的目录**（通常是项目根或工作区根，而不是 `st_taskcfg` 本身）。

Harbor 会从每个 search path **向下最多 5 层**查找：

- `{path}/**/st_taskcfg/tasks/` → 任务 YAML
- `{path}/**/st_taskcfg/groups/` → 组 YAML

示例：若存在 `/home/seen-e-embodied/wh_workspace/my-app/st_taskcfg/tasks/`，将

```json
"/home/seen-e-embodied/wh_workspace"
```

加入 `search_paths` 即可被发现。

路径写法：

- 推荐绝对路径：`/home/user/projects/foo`
- 也支持：`~/projects/foo`（展开为当前用户 HOME）

**不要**重复添加相同路径；**不要**添加不存在的目录。

## 修改方式（任选其一）

### 1. 直接编辑配置文件（推荐给 Agent）

1. 编辑 `~/.harbor/settings.json`，在 `search_paths` 数组中加入目录
2. **重启 Harbor**，或在 TaskClick 打开 Setting 弹层点一次 save（会重载配置）
3. 在 TaskClick 的 paths panel 点 **research**，扫描新路径下的 `st_taskcfg`

仅改文件且 Harbor 仍在运行时，内存中的配置不会自动更新，必须重启或经 Setting 保存。

### 2. TaskClick UI

1. TaskClick 顶栏 ⚙ **Setting** → **search paths**（多行文本，一行一个路径）→ save
2. TaskClick paths panel（文件夹图标）→ **research**

### 3. TaskClick paths panel 快捷添加

paths panel 里输入路径点 `+`，会写入 `settings.json` 并立即生效，无需手改文件。

## 示例：为 wh_workspace 启用项目内 TaskCard

```json
{
  "taskcard_root": "~/.harbor/st_taskcfg",
  "search_paths": [
    "/home/seen-e-embodied/wh_workspace"
  ],
  "metrics_fast_ms": 1000,
  "metrics_slow_ms": 10000
}
```

保存后重启 Harbor（或 Setting 里 save），再在 TaskClick 点 **research**。  
若项目内已有 `st_taskcfg/tasks/` 或 `st_taskcfg/groups/`，TaskClick 会按目录名分类显示。

## 与 taskcard_root 的区别

| | `taskcard_root` | `search_paths` |
|--|-----------------|----------------|
| 用途 | Harbor 全局默认 Root 任务目录 | 额外项目/worktree 根目录 |
| 典型路径 | `~/.harbor/st_taskcfg` | `/home/user/my-project` |
| YAML 位置 | `{taskcard_root}/tasks/` | `{search_path}/.../st_taskcfg/tasks/` |

改仓库内任务时，优先在项目根下维护 `st_taskcfg/`，并把**项目根**（不是 `st_taskcfg`）加入 `search_paths`。详见 [taskcard/paths.md](taskcard/paths.md)。
