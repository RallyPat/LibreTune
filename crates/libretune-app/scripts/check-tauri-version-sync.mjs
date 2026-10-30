import { readFileSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// `tauri build` refuses to start when a Rust tauri crate and its npm
// counterpart are on different major.minor releases. That check only runs
// inside `tauri build`, which no PR-time CI job performs — the nightly build
// was the first (and slowest) place to catch the drift (issue #345).
// This script performs the same comparison directly on the lockfiles so a
// mismatch fails fast at PR time.

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, '..');
const repoRoot = path.resolve(appRoot, '..', '..');

// [crate, npm package] pairs that `tauri build` compares. Extend this list
// if a plugin with a JS side (e.g. window-state) is ever adopted.
const pairs = [
  ['tauri', '@tauri-apps/api'],
  ['tauri-plugin-dialog', '@tauri-apps/plugin-dialog'],
  ['tauri-plugin-opener', '@tauri-apps/plugin-opener'],
];

function crateVersion(crate) {
  // Normalize line endings: the lockfile may be checked out with CRLF on
  // Windows, and the multi-line regex below matches on \n only.
  const lock = readFileSync(path.join(repoRoot, 'Cargo.lock'), 'utf8').replace(/\r\n/g, '\n');
  const re = new RegExp(`^\\[\\[package\\]\\]\\nname = "${crate}"\\nversion = "([^"]+)"`, 'm');
  const match = lock.match(re);
  if (!match) {
    throw new Error(`${crate} not found in Cargo.lock`);
  }
  return match[1];
}

function packageVersion(pkg) {
  const lock = JSON.parse(readFileSync(path.join(appRoot, 'package-lock.json'), 'utf8'));
  const entry = lock.packages?.[`node_modules/${pkg}`];
  if (!entry) {
    throw new Error(`${pkg} not found in package-lock.json`);
  }
  return entry.version;
}

function majorMinor(version) {
  const [major, minor] = version.split('.');
  return `${major}.${minor}`;
}

let mismatched = false;
for (const [crate, pkg] of pairs) {
  const crateVer = crateVersion(crate);
  const pkgVer = packageVersion(pkg);
  const inSync = majorMinor(crateVer) === majorMinor(pkgVer);
  console.log(`${inSync ? 'OK ' : 'BAD'} ${crate} ${crateVer} <-> ${pkg} ${pkgVer}`);
  if (!inSync) {
    mismatched = true;
  }
}

if (mismatched) {
  console.error(
    [
      '',
      'Tauri crate/npm version mismatch: `tauri build` rejects mixed',
      'major.minor releases of the Rust crates and the @tauri-apps packages.',
      'Re-align the Rust side to the npm side with:',
      '',
      '  cargo update -p tauri --precise <@tauri-apps/api version>',
      '  cargo update -p tauri-plugin-dialog --precise <plugin-dialog version>',
      '  cargo update -p tauri-plugin-opener --precise <plugin-opener version>',
      '',
    ].join('\n'),
  );
  process.exit(1);
}
