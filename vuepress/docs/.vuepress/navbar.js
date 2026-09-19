import { defineNavbarConfig } from 'vuepress-theme-plume'

export const navbar = defineNavbarConfig([
  { text: '首页', link: '/' },
  { text: '使用指南', link: '/guide/' },
  { text: '程序接入', link: '/integration/' },
  { text: '配置参考', link: '/reference/' },
  { text: '运维与开发', link: '/operations/' },
])
