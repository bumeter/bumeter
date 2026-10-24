// Bundle the extension into one CommonJS file; `vscode` is provided by the editor at run time.
import { build, context } from 'esbuild';

const options = { entryPoints: ['src/extension.ts'], bundle: true, platform: 'node', format: 'cjs', target: 'node18', outfile: 'dist/extension.js', external: ['vscode'], sourcemap: true, logLevel: 'info' };
if (process.argv.includes('--watch')) (await context(options)).watch();
else await build(options);
