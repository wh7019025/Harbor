# 创建 Task / Group

Agent 应直接创建目录和 YAML 文件，不需要操作 Harbor 界面。

## 阶段一：确定配置位置

- 项目配置：`{project}/harbor_taskcfg/`
- 当前机器的全局配置：`~/.harbor/harbor_taskcfg/`
- `taskcard_root` 被修改时，全局配置使用其实际路径

项目配置尚未被 Harbor 发现时，按 [../settings.md](../settings.md) 将项目根或工作区根加入 `search_paths`。

## 阶段二：创建 Task

1. 运行 `harbor --version` 获取当前版本。
2. 创建 `harbor_taskcfg/tasks/` 目录（若不存在）。
3. 按 [../yaml/task.md](../yaml/task.md) 写入 `{id}.yaml`。
4. 检查 `id`、`workdir`、`version`，并确认 `command` 使用 `argv` 或 `shell` + `script` 其中一种形式。

文件名建议与 Task `id` 一致，例如：

```text
harbor_taskcfg/tasks/build-assets.yaml
```

## 阶段三：创建 Group

1. 先确认 Group 引用的所有 Task 均已存在。
2. 创建 `harbor_taskcfg/groups/` 目录（若不存在）。
3. 按 [../yaml/group.md](../yaml/group.md) 写入 `{id}.yaml`。
4. 存在跨项目同名 Task 时，为对应条目填写 `prefix_path`。

文件名建议与 Group `id` 一致，例如：

```text
harbor_taskcfg/groups/dev-pipeline.yaml
```

## 阶段四：完成检查

- 确认 YAML 可以解析，且没有写入自动生成的顶层 `folder`、`prefix_path` 或 `taskcfg_dir`。
- 不要覆盖同目录下已有的同名配置；修改已有配置时保留无关字段。
- 完成后告知用户重新加载配置或重启 Harbor。
