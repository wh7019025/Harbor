# Group YAML

```yaml
version: 1
id: system-info
name: System Info
tasks:
  - task: uc-info
    wait_after_sec: 0
  - task: demo-ping
    # 未写 prefix_path → 按短 id 取第一个命中的 task
    wait_after_sec: 0
  - task: demo-ping
    prefix_path: /home/se/Harbor
    wait_after_sec: 0
```

- `tasks[].task` 必须引用已存在的 task 短 id
- `tasks[].prefix_path` 可选：工程绝对路径；已写则精确匹配该工程下的 task；未写则按加载顺序取第一个同名 task
- 跨工程允许同名 group `id`；同一 `st_taskcfg/groups/`（或 Root `groups/`）目录内短 id 仍唯一
- 新建 group 时 Folder 同样从 search path 选择（写入 `st_taskcfg/groups/`）
