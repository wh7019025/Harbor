# Harbor MCP

简单只读 MCP Resources：暴露 `~/.harbor/agent_doc/` 下的知识文件（含子目录）。零依赖，仅需 Node。

## Cursor 配置

应用启动后会生成 `~/.harbor/mcp.example.json`，把其中 `mcpServers.harbor` 合并进 Cursor MCP 配置：

```json
{
  "mcpServers": {
    "harbor": {
      "command": "node",
      "args": ["/home/YOU/.harbor/mcp/index.mjs"]
    }
  }
}
```

`index.mjs` 也会在每次启动时同步到 `~/.harbor/mcp/`。

## Resources

URI 形式：`harbor://agent_doc/<相对路径>`（POSIX `/`，含子目录）。

示例：

- `harbor://agent_doc/AgentDoc.md` — 结构树索引
- `harbor://agent_doc/yaml/task.md`
- `harbor://agent_doc/taskcard/paths.md`
- `harbor://agent_doc/MCP.md`
