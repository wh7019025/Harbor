import { defineCollection, defineCollections } from 'vuepress-theme-plume'

const guide = defineCollection({
  type: 'doc',
  dir: 'guide',
  title: '使用指南',
  sidebar: ['', 'concepts', 'getting-started', 'workspaces', 'tasks', 'groups', 'logs', 'panels', 'remote'],
})

const reference = defineCollection({
  type: 'doc',
  dir: 'reference',
  title: '配置参考',
  sidebar: ['', 'task-yaml', 'group-yaml', 'settings', 'web-api'],
})

const integration = defineCollection({
  type: 'doc',
  dir: 'integration',
  title: '程序接入',
  sidebar: ['', 'agent-skill', 'task-definition', 'runtime', 'web-panel', 'ros2'],
})

const operations = defineCollection({
  type: 'doc',
  dir: 'operations',
  title: '运维与开发',
  sidebar: ['', 'core', 'security', 'troubleshooting', 'build', 'architecture'],
})

export const collections = defineCollections([guide, integration, reference, operations])
