/* eslint-env node */
/* eslint-disable @typescript-eslint/no-var-requires */
// @ts-check
// Note: type annotations allow type checking and IDEs autocompletion

const path = require('path');

const lightCodeTheme = require('prism-react-renderer/themes/github');
const darkCodeTheme = require('prism-react-renderer/themes/dracula');

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'Blyss Docs',
  tagline: 'Retrieve data privately using homomorphic encryption.',
  url: 'https://docs.blyss.dev',
  baseUrl: '/',
  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',
  favicon: 'img/favicon.ico',

  i18n: {
    defaultLocale: 'en',
    locales: ['en']
  },

  presets: [
    [
      '@docusaurus/preset-classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      {
        docs: {
          sidebarPath: require.resolve('./sidebars.js'),
          routeBasePath: 'docs'
        },
        blog: false,
        theme: {
          customCss: require.resolve('./src/css/custom.css')
        }
      }
    ]
  ],

  plugins: [
    [
      'docusaurus-plugin-typedoc-api',

      // Plugin / TypeDoc options
      {
        projectRoot: path.join(__dirname, '../'),
        packages: [
          {
            path: '.',
            entry: 'js/index.ts'
          }
        ],
        gitRefName: 'main',
        typedocOptions: {
          excludePrivate: true
        }
      }
    ]
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    {
      navbar: {
        title: 'blyss',
        items: [
          {
            to: 'docs',
            label: 'Docs',
            position: 'left'
          },
          {
            to: 'api',
            label: 'API',
            position: 'left'
          }
        ]
      },
      footer: {
        style: 'dark',
        copyright: `Copyright © ${new Date().getFullYear()}, Blyss`
      },
      prism: {
        theme: lightCodeTheme,
        darkTheme: darkCodeTheme
      }
    }
};

module.exports = config;
