import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'tsk',
  description: 'Local-first, end-to-end encrypted task manager documentation',
  cleanUrls: true,
  themeConfig: {
    nav: [
      { text: 'Guide', link: '/overview' },
      { text: 'CLI', link: '/cli' },
      { text: 'Development', link: '/development/' },
      { text: 'Releases', link: '/releases' }
    ],
    sidebar: [
      { text: 'Overview', link: '/overview' },
      { text: 'Architecture', link: '/architecture' },
      { text: 'Security model', link: '/security' },
      { text: 'CLI', link: '/cli' },
      {
        text: 'Development',
        items: [
          { text: 'Linux app', link: '/development/linux' },
          { text: 'iOS app', link: '/development/ios' },
          { text: 'Server', link: '/development/server' }
        ]
      },
      { text: 'Releases', link: '/releases' },
      { text: 'Roadmap', link: '/roadmap' }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/matthewyjiang/tasks' }
    ],
    search: { provider: 'local' }
  }
})
