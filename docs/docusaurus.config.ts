import { themes as prismThemes } from 'prism-react-renderer';
import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'fyrer',
  tagline: 'Streaming DAG task orchestrator for polyglot monorepos — Rust, Go, TypeScript, Python.',
  favicon: 'img/fyrer/fyrer-dark.png',
  url: 'https://fyrer.vinm.me',
  baseUrl: '/',
  organizationName: '07calc',
  projectName: 'fyrer',
  onBrokenLinks: 'throw',
  headTags: [
    { tagName: 'link', attributes: { rel: 'preconnect', href: 'https://fonts.googleapis.com' } },
    { tagName: 'link', attributes: { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: 'anonymous' } },
    {
      tagName: 'link',
      attributes: {
        rel: 'stylesheet',
        href: 'https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600;700&family=Geist+Mono:wght@400;500&family=JetBrains+Mono:wght@400;500&family=Inter:wght@400;500;600;700&display=swap',
      },
    },
  ],
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },
  future: { v4: true },
  i18n: { defaultLocale: 'en', locales: ['en'] },
  presets: [
    [
      'classic',
      {
        docs: {
          path: 'docs',
          routeBasePath: '/docs',
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/07calc/fyrer/tree/main/docs/',
          showLastUpdateTime: false,
          showLastUpdateAuthor: false,
          breadcrumbs: true,
        },
        blog: false,
        theme: { customCss: './src/css/custom.css' },
      } satisfies Preset.Options,
    ],
  ],
  themeConfig: {
    image: 'img/fyrer-social-card.jpg',
    metadata: [
      { name: 'keywords', content: 'monorepo, task runner, rust, build system, caching, dag, fyrer, turborepo alternative' },
      { name: 'author', content: 'fyrer' },
      { name: 'theme-color', content: '#08090a' },
    ],
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: true,
      respectPrefersColorScheme: false,
    },
    docs: {
      sidebar: {
        hideable: true,
        autoCollapseCategories: false,
      },
    },
    tableOfContents: {
      minHeadingLevel: 2,
      maxHeadingLevel: 4,
    },
    navbar: {
      title: 'fyrer',
      logo: {
        alt: 'fyrer logo',
        src: 'img/fyrer/fyrer-dark.png',
        srcDark: 'img/fyrer/fyrer-dark.png',
        width: 22,
        height: 22,
      },
      hideOnScroll: false,
      items: [
        { type: 'docSidebar', sidebarId: 'fyrerSidebar', position: 'left', label: 'Docs' },
        { href: 'https://github.com/07calc/fyrer/releases', label: 'Changelog', position: 'left' },
        {
          href: 'https://github.com/07calc/fyrer',
          label: 'GitHub',
          position: 'right',
          className: 'navbar-cta navbar-github-icon',
          'aria-label': 'GitHub repository' as any,
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            { label: 'Introduction', to: '/docs/introduction' },
            { label: 'Quickstart', to: '/docs/quickstart' },
            { label: 'Configuration', to: '/docs/configuration/overview' },
          ],
        },
        {
          title: 'Community',
          items: [
            { label: 'GitHub', href: 'https://github.com/07calc/fyrer' },
            { label: 'Issues', href: 'https://github.com/07calc/fyrer/issues' },
            { label: 'Discussions', href: 'https://github.com/07calc/fyrer/discussions' },
          ],
        },
        {
          title: 'More',
          items: [
            { label: 'Releases', href: 'https://github.com/07calc/fyrer/releases' },
            { label: 'License', href: 'https://github.com/07calc/fyrer/blob/main/LICENSE' },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} fyrer. MIT — Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.vsDark,
      darkTheme: prismThemes.vsDark,
      additionalLanguages: ['yaml', 'bash', 'toml', 'json', 'rust', 'go', 'python'],
      magicComments: [
        { className: 'theme-code-block-highlighted-line', line: 'highlight-next-line', block: { start: 'highlight-start', end: 'highlight-end' } },
      ],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
