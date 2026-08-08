# 配置发现设置（settings.json）

配置文件路径：`~/.harbor/settings.json`

Agent 只需关注 `taskcard_root` 和 `search_paths`。其他 Harbor 设置与 Task / Group 配置维护无关，应保持原值。

```json
{
  "taskcard_root": "~/.harbor/harbor_taskcfg",
  "search_paths": [
    "/home/user/projects"
  ]
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `taskcard_root` | string | 全局 Task / Group 配置根目录；支持绝对路径、`~/...` 或相对 HOME 的路径 |
| `search_paths` | string[] | Harbor 搜索项目配置的起点目录 |

文件不存在时，Harbor 使用内置默认值：空 `search_paths`，`taskcard_root` 为 `~/.harbor/harbor_taskcfg`。

## Search paths

每一项必须是已存在的目录，通常填写项目根或工作区根，而不是 `harbor_taskcfg` 本身。Harbor 从每一项向下最多 5 层查找：

- `{path}/**/harbor_taskcfg/tasks/`
- `{path}/**/harbor_taskcfg/groups/`

路径支持绝对路径、`~/...`，以及相对 HOME 的路径。不要重复添加相同路径，也不要添加不存在的目录。

## Agent 修改规则

1. 读取现有 `settings.json`。
2. 仅向 `search_paths` 添加发现当前项目所必需的目录。
3. 保留所有未知字段，不要用示例覆盖整个文件。
4. 修改后告知用户重新加载配置或重启 Harbor；无需描述界面操作。

项目内配置应优先维护在项目根的 `harbor_taskcfg/`，并将项目根或包含它的工作区根加入 `search_paths`。
