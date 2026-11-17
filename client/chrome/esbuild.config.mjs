import * as esbuild from 'esbuild';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const entryPoints = [
  join('src', 'background', 'background.ts'),
  join('src', 'content', 'content.ts'),
  join('src', 'options', 'options.ts'),
  join('src', 'offscreen', 'offscreen.ts'),
  join('src', 'sidepanel', 'sidepanel.ts'),
];

const preserveDynamicImportsPlugin = {
  name: 'preserve-dynamic-imports',
  setup(build) {
    build.onResolve({ filter: /^chrome-extension:/ }, args => ({
      path: args.path,
      external: true,
    }));
  },
};

export function createBuildOptions({ outdir = 'dist', dev = false } = {}) {
  return {
    entryPoints,
    bundle: true,
    outdir,
    outbase: 'src',
    format: 'esm',
    target: 'es2020',
    sourcemap: dev,
    minify: !dev,
    loader: {
      '.ts': 'ts',
      '.tsx': 'tsx',
    },
    plugins: [preserveDynamicImportsPlugin],
  };
}

export async function runEsbuild({ outdir = 'dist', dev = false, watch = false } = {}) {
  const options = createBuildOptions({ outdir, dev });

  if (watch) {
    const context = await esbuild.context(options);
    await context.watch();
    return context;
  }

  await esbuild.build(options);
  return null;
}

const currentFile = fileURLToPath(import.meta.url);
const isDirectRun = process.argv[1] === currentFile;

if (isDirectRun) {
  const outdirArg = process.argv.find(arg => arg.startsWith('--outdir='));
  const outdir = outdirArg ? outdirArg.split('=')[1] : 'dist';
  const dev = process.argv.includes('--dev');
  const watch = process.argv.includes('--watch');
  await runEsbuild({ outdir, dev, watch });
}
