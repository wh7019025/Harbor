# SuperTerm MCP

简单只读 MCP Resources：暴露 `~/.superterm/agent_doc/` 下的知识文件（含子目录）。零依赖，仅需 Node。

## Cursor 配置

应用启动后会生成 `~/.superterm/mcp.example.json`，把其中 `mcpServers.superterm` 合并进 Cursor MCP 配置：

```json
{
  "mcpServers": {
    "superterm": {
      "command": "node",
      "args": ["/home/YOU/.superterm/mcp/index.mjs"]
    }
  }
}
```

`index.mjs` 也会在每次启动时同步到 `~/.superterm/mcp/`。

## Resources

URI 形式：`superterm://agent_doc/<相对路径>`（POSIX `/`，含子目录）。

示例：

- `superterm://agent_doc/AgentDoc.md` — 结构树索引
- `superterm://agent_doc/yaml/task.md`
- `superterm://agent_doc/taskcard/paths.md`
- `superterm://agent_doc/MCP.md`
