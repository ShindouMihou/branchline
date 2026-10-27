import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve, join, delimiter } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const local = resolve(root, '.local/toolchain');
const env = { ...process.env };
if (!env.CARGO_HOME && existsSync(join(local, 'cargo/bin/cargo'))) {
  env.CARGO_HOME = join(local, 'cargo'); env.RUSTUP_HOME = join(local, 'rustup');
}
const cargoBin = join(env.CARGO_HOME || join(homedir(), '.cargo'), 'bin');
env.PATH = cargoBin + delimiter + (env.PATH || '');
const result = spawnSync(join(root, 'node_modules/.bin/tauri'), process.argv.slice(2), { cwd: root, env, stdio: 'inherit' });
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);
