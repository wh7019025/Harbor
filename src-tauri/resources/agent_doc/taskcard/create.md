# 创建 Task 或 Group

Task 和 Group 是两种独立配置。Agent 只创建用户明确要求的类型：请求创建 Task 时不要附带创建 Group；只有用户明确要求组合、编排或创建 Group 时才创建 Group。

Agent 应直接创建目录和 YAML 文件，不需要操作 Harbor 界面。

## 公共准备：确定配置位置

- 项目配置：`{project}/harbor_taskcfg/`
- 当前机器的全局配置：`~/.harbor/harbor_taskcfg/`
- `taskcard_root` 被修改时，全局配置使用其实际路径

创建项目配置前，Agent 必须按 [../settings.md](../settings.md) 检查 Search Paths。项目尚未被覆盖时，直接将项目根加入 `search_paths`，同时保留其他设置字段。

## 创建 Task

仅在用户要求创建 Task 时执行：

1. 运行 `harbor --version` 获取当前版本。
2. 创建 `harbor_taskcfg/tasks/` 目录（若不存在）。
3. 按 [../yaml/task.md](../yaml/task.md) 写入 `{id}.yaml`。
4. 检查 `id`、`workdir`、`version`，并确认 `command` 使用 `argv` 或 `shell` + `script` 其中一种形式。

文件名建议与 Task `id` 一致，例如：

```text
harbor_taskcfg/tasks/build-assets.yaml
```

完成 Task 后停止，不要自行创建 Group。

## 创建 Group

仅在用户明确要求创建 Group、组合任务或编排已有 Task 时执行：

1. 先确认 Group 引用的所有 Task 均已存在；不要为了填充 Group 而自行创建用户未要求的 Task。
2. 创建 `harbor_taskcfg/groups/` 目录（若不存在）。
3. 按 [../yaml/group.md](../yaml/group.md) 写入 `{id}.yaml`。
4. Group 仅填写 Task `id`；若跨目录搜索得到多个同名 Task，Harbor 会拒绝保存并列出候选位置。

文件名建议与 Group `id` 一致，例如：

```text
harbor_taskcfg/groups/dev-pipeline.yaml
```

## 完成检查

- 只检查和交付用户要求的配置类型。
- 确认 YAML 可以解析，且没有写入自动生成的顶层 `folder`、`prefix_path` 或 `taskcfg_dir`。
- 不要覆盖同目录下已有的同名配置；修改已有配置时保留无关字段。
- 完成后告知用户重新加载配置或重启 Harbor。
