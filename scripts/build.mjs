import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const cwd = fileURLToPath(new URL('../crates/api/', import.meta.url));
const run = (cmd, args) => {
  const result = spawnSync(cmd, args, { cwd, stdio: 'inherit', shell: false });
  if (result.error || result.status !== 0) process.exit(result.status || 1);
};
const installed = spawnSync('worker-build', ['--version'], { encoding: 'utf8' });
if (installed.status !== 0 || !installed.stdout.includes('0.8.5')) {
  run('cargo', ['install', 'worker-build', '--version', '0.8.5', '--locked']);
}
run('worker-build', ['--release']);
