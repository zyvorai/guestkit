import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'GuestKit',
  tagline: 'Offline VM intelligence. Migration assurance you can prove.',
  favicon: 'img/favicon.svg',

  future: {
    v4: true,
  },

  url: 'https://zyvorai.github.io',
  baseUrl: '/guestkit/',

  organizationName: 'zyvorai',
  projectName: 'guestkit',

  onBrokenLinks: 'warn',

  markdown: {
    format: 'md',
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          path: '../docs',
          routeBasePath: 'docs',
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/zyvorai/guestkit/tree/main/docs/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      hideOnScroll: false,
      title: 'GuestKit',
      logo: {
        alt: 'GuestKit',
        src: 'img/favicon.svg',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'right',
          label: 'Docs',
        },
        {
          href: 'https://github.com/zyvorai/guestkit',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            {label: 'Getting started', to: '/docs/user-guides/getting-started'},
            {label: 'Quick reference', to: '/docs/user-guides/quick-reference'},
            {label: 'Open source vs Enterprise', to: '/docs/ce-vs-enterprise'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'GitHub', href: 'https://github.com/zyvorai/guestkit'},
            {label: 'crates.io', href: 'https://crates.io/crates/guestkit'},
            {label: 'License (Apache-2.0)', href: 'https://github.com/zyvorai/guestkit/blob/main/LICENSE'},
          ],
        },
        {
          title: 'Zyvor Enterprise',
          items: [
            {label: '30-day Enterprise trial', href: 'https://github.com/zyvorai/guestkit/blob/main/docs/enterprise-trial-install.md'},
            {label: 'Book a demo', href: 'https://zyvor.dev/contact?utm_source=github&utm_medium=guestkit&intent=demo'},
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Zyvor. GuestKit core is Apache-2.0 licensed.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
