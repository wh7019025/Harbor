import { invoke } from "@tauri-apps/api/core";

export interface AgentHelpInfo {
  home: string;
  agent_doc_dir: string;
  files: string[];
  prompt: string;
  mcp_example: string;
}

export function getAgentHelp() {
  return invoke<AgentHelpInfo>("get_agent_help");
}

export function refreshAgentDoc() {
  return invoke<AgentHelpInfo>("refresh_agent_doc");
}
