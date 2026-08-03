import type { StorybookConfig } from '@storybook/svelte-vite';
import type { Plugin } from 'vite';
import { resolve } from 'path';
import { aliases } from '../aliases.ts';

function mockWsPlugin(): Plugin {
  const mockPath = resolve(process.cwd(), 'src/stories/mockWs.ts');
  return {
    name: 'mock-ws-plugin',
    enforce: 'pre',
    resolveId(source) {
      // Intercept both the alias and the resolved path
      if (source === '@/lib/ws' || source.endsWith('/src/lib/ws.ts') || source.endsWith('/src/lib/ws')) {
        return mockPath;
      }
      return null;
    },
  };
}

const config: StorybookConfig = {
  stories: ['../src/**/*.mdx', '../src/**/*.stories.@(js|ts|svelte)'],
  addons: [
    '@storybook/addon-links',
    '@storybook/addon-essentials',
    '@storybook/addon-interactions',
  ],
  framework: {
    name: '@storybook/svelte-vite',
    options: {},
  },
  viteFinal: async (viteConfig) => {
    viteConfig.plugins = viteConfig.plugins || [];
    viteConfig.plugins.push(mockWsPlugin());
    viteConfig.resolve = viteConfig.resolve || {};
    viteConfig.resolve.alias = {
      ...viteConfig.resolve.alias,
      ...aliases,
    };
    return viteConfig;
  },
};

export default config;
