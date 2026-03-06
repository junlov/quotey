import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    {
      type: 'category',
      label: 'Introduction',
      collapsed: false,
      items: [
        'intro/what-is-quotey',
        'intro/getting-started',
        'intro/quick-start-guide',
        'intro/key-concepts',
      ],
    },
    {
      type: 'category',
      label: 'Architecture',
      collapsed: false,
      items: [
        'architecture/overview',
        'architecture/six-box-model',
        'architecture/data-flow',
        'architecture/safety-principle',
        'architecture/technology-stack',
      ],
    },
    {
      type: 'category',
      label: 'Core Concepts',
      collapsed: false,
      items: [
        'core-concepts/cpq-engine',
        'core-concepts/flow-engine',
        'core-concepts/determinism',
        'core-concepts/audit-trail',
        'core-concepts/quote-lifecycle',
      ],
    },
    {
      type: 'category',
      label: 'Crate Reference',
      collapsed: true,
      items: [
        'crates/overview',
        'crates/core',
        'crates/db',
        'crates/agent',
        'crates/slack',
        'crates/mcp',
        'crates/cli',
        'crates/server',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      collapsed: true,
      items: [
        'guides/configuration',
        'guides/database-migrations',
        'guides/slack-setup',
        'guides/llm-configuration',
        'guides/crm-integration',
        'guides/pdf-templates',
        'guides/testing',
      ],
    },
    {
      type: 'category',
      label: 'Advanced Features',
      collapsed: true,
      items: [
        'advanced/deal-dna',
        'advanced/autopsy-revenue-genome',
        'advanced/policy-optimizer',
        'advanced/negotiation-autopilot',
        'advanced/precedent-intelligence',
      ],
    },
    {
      type: 'category',
      label: 'API Reference',
      collapsed: true,
      items: [
        'api/mcp-tools',
        'api/slack-commands',
        'api/cli-commands',
        'api/database-schema',
      ],
    },
    {
      type: 'category',
      label: 'Contributing',
      collapsed: true,
      items: [
        'contributing/overview',
        'contributing/development-setup',
        'contributing/code-style',
        'contributing/testing-guide',
        'contributing/documentation',
      ],
    },
  ],
};

export default sidebars;
