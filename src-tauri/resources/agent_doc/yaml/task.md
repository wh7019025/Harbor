# Task YAML

```yaml
version: 1
id: demo-ping
name: Demo Ping
workdir: /home/se/SuperTerm
command:
  shell: sh
  script: |
    echo hello
    sleep 1
```

或 argv 形式：

```yaml
version: 1
id: uname-kernel
name: Uname Kernel
workdir: ~
command:
  argv:
    - uname
    - -a
```

规则：

- `version` 必须为 `1`
- `id`：字母数字、`-`、`_`（短 id；YAML 内不写工程路径）
- 跨工程允许同名 `id`；同一 `st_taskcfg/tasks/`（或 Root `tasks/`）目录内短 id 仍唯一
- 运行时身份为 `(prefix_path, id)`：`prefix_path` 为工程绝对路径（Root 为 `taskcard_root`；项目为 `st_taskcfg` 的父目录）
- `workdir`：工作目录，可用 `~`
- `sudo: true` 时启动需输入密码
- 不要把 `folder` / `prefix_path` 写进 YAML（由存放位置决定）
