# Search Paths 维护（settings.json）

配置文件路径：`~/.harbor/settings.json`

Search Paths 是 Agent 创建和维护项目 Task / Group 配置的一部分。Agent 可以直接维护 `search_paths`，无需指导用户操作界面。

Agent 只应修改 `taskcard_root` 和 `search_paths`；其他 Harbor 设置必须保留原值。

```json
{
  "taskcard_root": "~/.harbor/harbor_taskcfg",
  "search_paths": [
    "/home/user/projects/my-project"
  ]
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `taskcard_root` | string | 全局 Task / Group 配置根目录；支持绝对路径、`~/...` 或相对 HOME 的路径 |
| `search_paths` | string[] | Harbor 搜索项目配置的起点目录 |

文件不存在时，Harbor 使用内置默认值：空 `search_paths`，`taskcard_root` 为 `~/.harbor/harbor_taskcfg`。

## 发现规则

每个 Search Path 必须是已存在的目录。Harbor 从该目录向下最多 5 层查找：

- `{search_path}/**/harbor_taskcfg/tasks/`
- `{search_path}/**/harbor_taskcfg/groups/`

应填写项目根或工作区根，而不是 `harbor_taskcfg` 本身。默认优先填写当前项目根，避免使用 `/`、HOME 或包含大量无关仓库的宽泛目录。

路径支持绝对路径、`~/...`，以及相对 HOME 的路径。

## Agent 维护 pipeline

### 阶段一：读取与检查

1. 读取现有 `~/.harbor/settings.json`；文件不存在时以 Harbor 默认值为基础创建。
2. 确认当前项目的绝对路径和 `harbor_taskcfg` 位置。
3. 检查现有 `search_paths` 是否已经覆盖该项目，并确认 `harbor_taskcfg` 位于向下 5 层以内。
4. 已覆盖时不要重复添加。

### 阶段二：添加路径

1. 选择能够发现配置的最窄实用目录，通常是项目根。
2. 确认目录真实存在。
3. 将规范化后的路径追加到 `search_paths`，并按规范化路径去重。
4. 保留 `settings.json` 的全部未知字段和值，不要用文档示例覆盖整个文件。

### 阶段三：删除路径

仅在用户明确要求删除，或已确认路径失效且不会影响其他项目配置时移除。不要因为当前任务未使用某条 Search Path 就擅自删除。

### 阶段四：验证与生效

1. 确认 JSON 可以解析，`search_paths` 是字符串数组且没有重复项。
2. 确认目标路径下的 `harbor_taskcfg/tasks/` 或 `harbor_taskcfg/groups/` 能在 5 层范围内被发现。
3. 告知用户重新加载配置或重启 Harbor，使直接编辑的设置生效。

## 示例

当前项目为 `/home/user/projects/my-project`，配置位于：

```text
/home/user/projects/my-project/harbor_taskcfg/tasks/
```

推荐添加：

```json
"/home/user/projects/my-project"
```

不要添加：

```json
"/home/user/projects/my-project/harbor_taskcfg"
```
