import { defineCollection, defineCollections } from 'vuepress-theme-plume'

const guide = defineCollection({
  type: 'doc',
  dir: 'guide',
  title: '使用指南',
  sidebar: [
    { text: '什么是 Harbor', link: '/guide/' },
    'concepts',
    'getting-started',
    'work-with-ai',
    'workspaces',
    'tasks',
    'groups',
    'logs',
    'panels',
    'remote',
    {
      text: '配置参考',
      collapsed: false,
      items: ['reference/', 'reference/task-yaml', 'reference/group-yaml', 'reference/settings', 'reference/web-api'],
    },
  ],
})

const development = defineCollection({
  type: 'doc',
  dir: 'development',
  title: '开发',
  sidebar: [{ text: '设计理念', link: '/development/' }, 'architecture', 'environment', 'build', 'security'],
})

export const collections = defineCollections([guide, development])
