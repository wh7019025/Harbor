---
title: troubleshooting
createTime: 2026/09/19 23:57:12
permalink: /operations/0pzs445d/
---
# 故障排查

## GUI 先白屏再显示

开发模式下可能是前端资源、Core 探测或初始化请求阻塞。先运行 `npm run build` 排除前端编译错误，再查看开发终端和 Core 日志。

## 切换远端卡住

按顺序检查：

1. SSH 是否可在终端快速连接。
2. DNS 或 host 是否解析到预期地址。
3. TCP `29385` 是否可达。
4. Core hash、版本和 API revision 是否匹配。
5. 是否每次切换都触发复制；若是，检查远端目标文件是否保留、权限是否正确。

## matching release harbor_core not found

该错误表示 GUI 固化的 SHA-256 与候选二进制都不相同。重新执行 release Core 构建，再构建 GUI，确保 hash 由同一次构建产物生成。debug Core 不能替代 release Core。

## missing field `uuid`

旧 Core 在反序列化快照时可能要求 UUID，而新 GUI 期望自动补齐。先确认 API revision 一致，然后刷新发现。当前 Core 会为缺少 UUID 的 YAML 生成并写回 UUID。

## Qt 无法连接显示器

确认远端用户存在活动桌面会话，并检查：

```bash
echo "$DISPLAY"
echo "$XAUTHORITY"
echo "$WAYLAND_DISPLAY"
echo "$DBUS_SESSION_BUS_ADDRESS"
```

没有桌面会话时使用 VNC/RDP，或让应用使用 offscreen 后端。

## xdg-open status 4

远端路径不属于本机文件系统，`xdg-open` 无法直接打开。使用 SFTP 文件管理器、Remote SSH，或先挂载远端目录。

## Workspace 切换慢

缩小 `search_paths`，避免扫描大型目录；确认本地 Workspace 没有误配为远端；观察是否在每次切换时重复部署 Core。
