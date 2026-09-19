import { defineNavbarConfig } from 'vuepress-theme-plume'

export const navbar = defineNavbarConfig([
  { text: '首页', link: '/' },
  { text: '使用指南', link: '/guide/' },
  { text: '开发', link: '/development/' },
  { text: '下载', link: 'https://github.com/wh7019025/Harbor/releases' },
])
