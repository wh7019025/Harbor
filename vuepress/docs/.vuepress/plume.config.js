import { defineThemeConfig } from 'vuepress-theme-plume'
import { navbar } from './navbar'
import { collections } from './collections'

export default defineThemeConfig({
  logo: '/harbor.svg',
  base: '/',
  docsRepo: 'https://github.com/wh7019025/Harbor',
  docsDir: 'docs',
  appearance: 'force-dark',
  social: [
    { icon: 'github', link: 'https://github.com/wh7019025/Harbor' },
  ],
  profile: {
    avatar: '/harbor.svg',
    name: 'Harbor',
    description: '机器人任务编排与远端运行平台',
  },
  navbar,
  collections,
})
