import { invoke } from "@tauri-apps/api/core";

export interface AgentSkillInfo {
  skill_dir: string;
  files: string[];
  prompt: string;
}

export function getAgentSkill() {
  return invoke<AgentSkillInfo>("get_agent_skill");
}

export function refreshAgentSkill() {
  return invoke<AgentSkillInfo>("refresh_agent_skill");
}
