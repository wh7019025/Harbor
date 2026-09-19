import { defineUserConfig } from 'vuepress'
import { viteBundler } from '@vuepress/bundler-vite'
import { plumeTheme } from 'vuepress-theme-plume'

export default defineUserConfig({
  base: '/',
  lang: 'zh-CN',
  title: 'Harbor',
  description: '面向机器人开发的本地与远端任务编排工具',
  head: [
    ['meta', { name: 'theme-color', content: '#1677ff' }],
    ['link', { rel: 'icon', href: '/harbor.svg' }],
  ],
  bundler: viteBundler(),
  theme: plumeTheme({
    hostname: 'https://harbor.hyln.space/',
    markdown: {
      demo: true,
      math: { type: 'katex' },
    },
  }),
})
