import * as esbuild from 'esbuild';

const isWatch = process.argv.includes('--watch');
const isDev = process.argv.includes('--dev');

// Support custom output directory via --outdir=<path>
const outdirArg = process.argv.find(arg => arg.startsWith('--outdir='));
const outdir = outdirArg ? outdirArg.split('=')[1] : './dist/debug';

const entryPoints = [];
const srcDir = './src';

entryPoints.push(join(srcDir, 'background', 'background.ts'));
entryPoints.push(join(srcDir, 'content', 'content.ts'));
entryPoints.push(join(srcDir, 'options', 'options.ts'));
entryPoints.push(join(srcDir, 'offscreen', 'offscreen.ts'));
entryPoints.push(join(srcDir, 'sidepanel', 'sidepanel.ts'));

import { join } from 'path';

// Plugin to preserve dynamic imports for chrome-extension:// URLs
const preserveDynamicImportsPlugin = {
  name: 'preserve-dynamic-imports',
  setup(build) {
    // Mark chrome-extension:// imports as external
    build.onResolve({ filter: /^chrome-extension:/ }, (args) => {
      return { path: args.path, external: true };
    });
  },
};

const buildOptions = {
  entryPoints,
  bundle: true,
  outdir,
  outbase: './src',
  format: 'esm',
  target: 'es2020',
  sourcemap: isDev,
  minify: !isDev,
  loader: {
    '.ts': 'ts',
    '.tsx': 'tsx',
  },
  plugins: [preserveDynamicImportsPlugin],
};

async function build() {
  try {
    if (isWatch) {
      const ctx = await esbuild.context(buildOptions);
      await ctx.watch();
      console.log('Watching for changes...');
    } else {
      await esbuild.build(buildOptions);
      console.log('Build complete!');
    }
  } catch (error) {
    console.error('Build failed:', error);
    process.exit(1);
  }
}

build();
