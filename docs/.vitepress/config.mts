import { defineConfig } from 'vitepress';

const repo = 'https://github.com/Ujap1994/ElectronicWerk';

export default defineConfig({
  title: 'ElectronicWerk',
  description: 'Open-source ECAD suite for makers and hobbyists: documentation and user guide.',
  lang: 'en-US',
  base: '/ElectronicWerk/',
  cleanUrls: true,
  lastUpdated: true,
  srcExclude: ['adr/**'],
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/ElectronicWerk/icon.svg' }],
    ['meta', { name: 'theme-color', content: '#13795b' }],
  ],
  themeConfig: {
    logo: '/icon.svg',
    nav: [
      { text: 'Guide', link: '/guide/introduction' },
      { text: 'Reference', link: '/reference/cli' },
      { text: 'Development', link: '/development/architecture' },
      { text: 'Roadmap', link: `${repo}/blob/main/ROADMAP.md` },
      { text: 'Download', link: `${repo}/releases/latest` },
    ],
    sidebar: [
      {
        text: 'Getting started',
        items: [
          { text: 'Introduction', link: '/guide/introduction' },
          { text: 'Installation', link: '/guide/installation' },
          { text: 'First start', link: '/guide/getting-started' },
        ],
      },
      {
        text: 'Using ElectronicWerk',
        items: [
          { text: 'Projects', link: '/guide/projects' },
          { text: 'Project tree and properties', link: '/guide/project-tree' },
          { text: 'Viewing sheets and boards', link: '/guide/viewer' },
          { text: 'Undo, autosave and recovery', link: '/guide/undo-and-recovery' },
          { text: 'Settings', link: '/guide/settings' },
          { text: 'Command palette', link: '/guide/command-palette' },
          { text: 'Updates', link: '/guide/updates' },
          { text: 'Keyboard shortcuts', link: '/guide/keyboard-shortcuts' },
          { text: 'Troubleshooting', link: '/guide/troubleshooting' },
        ],
      },
      {
        text: 'Reference',
        items: [
          { text: 'Command-line tool (ewerk)', link: '/reference/cli' },
          { text: 'Engine API', link: '/reference/engine-api' },
          { text: 'File format', link: '/reference/file-format' },
        ],
      },
      {
        text: 'Development',
        items: [
          { text: 'Architecture', link: '/development/architecture' },
          { text: 'Building from source', link: '/development/building' },
          { text: 'Benchmarks', link: '/development/benchmarks' },
          { text: 'Releases', link: '/development/releases' },
        ],
      },
    ],
    socialLinks: [{ icon: 'github', link: repo }],
    search: { provider: 'local' },
    editLink: {
      pattern: `${repo}/edit/main/docs/:path`,
      text: 'Edit this page on GitHub',
    },
    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2026 ElectronicWerk contributors',
    },
    outline: { level: [2, 3] },
  },
});
