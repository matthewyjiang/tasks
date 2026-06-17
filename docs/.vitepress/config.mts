import { defineConfig } from 'vitepress'

export default defineConfig({
  base: '/tasks/',
  title: 'tsk',
  description: 'Local-first, end-to-end encrypted task manager documentation',
  cleanUrls: true,
  themeConfig: {
    nav: [
      { text: 'Get started', link: '/getting-started' },
      { text: 'Clients', link: '/clients/' },
      { text: 'CLI', link: '/cli' },
      { text: 'Reference', link: '/architecture' },
      { text: 'Contributing', link: '/development/' }
    ],
    sidebar: [
      {
        text: 'Start here',
        items: [
          { text: 'Overview', link: '/overview' },
          { text: 'Choose a client', link: '/getting-started' },
          { text: 'Known limitations', link: '/roadmap' }
        ]
      },
      {
        text: 'Clients',
        items: [
          { text: 'Client status', link: '/clients/' },
          { text: 'Linux app', link: '/clients/linux' },
          { text: 'iOS app', link: '/clients/ios' },
          { text: 'CLI', link: '/cli' }
        ]
      },
      {
        text: 'Operate',
        items: [
          { text: 'Server setup', link: '/server' },
          { text: 'Security model', link: '/security' }
        ]
      },
      {
        text: 'Reference',
        collapsed: true,
        items: [
          { text: 'Architecture and sync model', link: '/architecture' },
          { text: 'Release process', link: '/releases' }
        ]
      },
      {
        text: 'Contributing',
        collapsed: true,
        items: [
          { text: 'Development overview', link: '/development/' },
          { text: 'Linux development', link: '/development/linux' },
          { text: 'iOS development', link: '/development/ios' },
          { text: 'Server development', link: '/development/server' }
        ]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/matthewyjiang/tasks' }
    ],
    search: { provider: 'local' }
  }
})
