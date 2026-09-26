import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  // `style.configFile: false` keeps svelte-check from loading vite.config.ts
  // when resolving CSS preprocessing config — that config deliberately throws
  // at evaluation time when VITE_PROXY_LOGS_PATH is unset (dev-server fail
  // fast), which would break checking. Inside the real vite/vitest pipeline
  // this inline config is replaced with the actual resolved server config
  // (vite-plugin-svelte tags the preprocessor with it); the css.modules mirror
  // below keeps standalone preprocessing consistent with vite.config.ts.
  preprocess: vitePreprocess({
    style: {
      configFile: false,
      css: {
        modules: {
          localsConvention: 'camelCase'
        }
      }
    }
  }),
  compilerOptions: {
    runes: true
  }
};
