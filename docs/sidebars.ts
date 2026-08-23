import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  fyrerSidebar: [
    {
      type: 'category',
      label: 'Get Started',
      collapsed: false,
      collapsible: true,
      items: ['introduction', 'installation', 'quickstart'],
    },
    {
      type: 'category',
      label: 'Core Concepts',
      collapsed: false,
      collapsible: true,
      items: ['concepts/how-it-works', 'concepts/packages-and-tasks', 'concepts/dependency-graph', 'concepts/caching'],
    },
    {
      type: 'category',
      label: 'Configuration',
      collapsed: false,
      collapsible: true,
      items: ['configuration/overview', 'configuration/packages', 'configuration/tasks', 'configuration/environment'],
    },
    {
      type: 'category',
      label: 'CLI Reference',
      collapsed: false,
      collapsible: true,
      items: ['cli/run', 'cli/plan', 'cli/list'],
    },
    {
      type: 'category',
      label: 'Guides',
      collapsed: false,
      collapsible: true,
      items: ['guides/dev-servers', 'guides/ci-mode', 'guides/multi-language'],
    },
  ],
};

export default sidebars;
