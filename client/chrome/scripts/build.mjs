import { spawn, spawnSync } from 'node:child_process';
import { cp, copyFile, mkdir, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runEsbuild } from '../esbuild.config.mjs';

const root = fileURLToPath(new URL('..', import.meta.url));
const dist = join(root, 'dist');
const isWatch = process.argv.includes('--watch');
const isCssOnly = process.argv.includes('--css-only');
const isDev = process.argv.includes('--dev') || isWatch;

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function watch(command, args) {
  const child = spawn(command, args, {
    cwd: root,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });

  child.on('exit', code => {
    if (code && code !== 0) process.exit(code);
  });

  return child;
}

async function ensureOutputDirs() {
  await Promise.all([
    mkdir(join(dist, 'sidepanel'), { recursive: true }),
    mkdir(join(dist, 'options'), { recursive: true }),
    mkdir(join(dist, 'offscreen'), { recursive: true }),
  ]);
}

async function copyStatic() {
  await ensureOutputDirs();
  await Promise.all([
    copyFile(join(root, 'manifest.json'), join(dist, 'manifest.json')),
    copyFile(join(root, 'src/sidepanel/sidepanel.html'), join(dist, 'sidepanel/sidepanel.html')),
    copyFile(join(root, 'src/sidepanel/sidepanel.css'), join(dist, 'sidepanel/sidepanel.css')),
    copyFile(join(root, 'src/options/options.html'), join(dist, 'options/options.html')),
    copyFile(join(root, 'src/offscreen/offscreen.html'), join(dist, 'offscreen/offscreen.html')),
    cp(join(root, 'assets'), join(dist, 'assets'), { recursive: true }),
    cp(join(root, 'wasm/pkg'), join(dist, 'wasm/pkg'), { recursive: true }),
  ]);
}

async function buildCss() {
  await ensureOutputDirs();
  run('tailwindcss', ['-i', 'src/styles/globals.css', '-o', 'dist/sidepanel/styles.css']);
  await copyFile(join(dist, 'sidepanel/styles.css'), join(dist, 'options/styles.css'));
}

async function build() {
  await rm(dist, { recursive: true, force: true });
  run('wasm-pack', ['build', '--target', 'web', './wasm']);
  await runEsbuild({ outdir: 'dist', dev: isDev });
  await buildCss();
  await copyStatic();
}

async function dev() {
  await rm(dist, { recursive: true, force: true });
  run('wasm-pack', ['build', '--target', 'web', './wasm']);
  await copyStatic();
  await runEsbuild({ outdir: 'dist', dev: true, watch: true });
  const tailwind = watch('tailwindcss', ['-i', 'src/styles/globals.css', '-o', 'dist/sidepanel/styles.css', '--watch']);

  const stop = () => {
    tailwind.kill('SIGTERM');
    process.exit(0);
  };

  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
  await new Promise(() => {});
}

if (isCssOnly) {
  await buildCss();
} else if (isWatch) {
  await dev();
} else {
  await build();
}
