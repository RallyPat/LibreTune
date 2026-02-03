import { promises as fs } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, '..');
const repoRoot = path.resolve(appRoot, '..', '..');

const docsSrcDir = path.join(repoRoot, 'docs', 'src');
const docsScreensDir = path.join(repoRoot, 'docs', 'screenshots');
const publicManualDir = path.join(appRoot, 'public', 'manual');
const publicScreensDir = path.join(publicManualDir, 'screenshots');

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function copyDir(src, dest) {
  await ensureDir(dest);
  const entries = await fs.readdir(src, { withFileTypes: true });
  await Promise.all(
    entries.map(async (entry) => {
      const srcPath = path.join(src, entry.name);
      const destPath = path.join(dest, entry.name);
      if (entry.isDirectory()) {
        await copyDir(srcPath, destPath);
      } else if (entry.isFile()) {
        await fs.copyFile(srcPath, destPath);
      }
    })
  );
}

function normalizePath(link) {
  return link.replace(/^\.\//, '').replace(/\.md$/i, '').replace(/\\/g, '/');
}

function parseSummary(summaryContent) {
  const lines = summaryContent.split('\n');
  const toc = [];
  const stack = [];
  let currentHeading = null;

  for (const line of lines) {
    const headingMatch = line.match(/^#+\s+(.*)$/);
    if (headingMatch) {
      const title = headingMatch[1].trim();
      if (title.toLowerCase() === 'summary') {
        currentHeading = null;
        stack.length = 0;
        continue;
      }
      currentHeading = { title, path: null, children: [] };
      toc.push(currentHeading);
      stack.length = 0;
      continue;
    }

    const itemMatch = line.match(/^(\s*)-\s+\[(.+?)\]\((.+?)\)/);
    if (!itemMatch) continue;

    const indent = itemMatch[1].length;
    const depth = Math.floor(indent / 2);
    const title = itemMatch[2].trim();
    const rawPath = itemMatch[3].trim();
    const entry = {
      title,
      path: normalizePath(rawPath),
      children: [],
    };

    while (stack.length > 0 && stack[stack.length - 1].depth >= depth) {
      stack.pop();
    }

    let parent = null;
    if (depth === 0 && currentHeading) {
      parent = currentHeading;
    } else if (stack.length > 0) {
      parent = stack[stack.length - 1].entry;
    }

    if (parent) {
      parent.children.push(entry);
    } else {
      toc.push(entry);
    }

    stack.push({ depth, entry });
  }

  const fillGroupPaths = (entries) => {
    for (const entry of entries) {
      if (!entry.path && entry.children && entry.children.length > 0) {
        entry.path = entry.children[0].path || null;
      }
      if (entry.children) {
        fillGroupPaths(entry.children);
      }
    }
  };

  fillGroupPaths(toc);
  return toc;
}

async function writeTocJson() {
  const summaryPath = path.join(docsSrcDir, 'SUMMARY.md');
  const summary = await fs.readFile(summaryPath, 'utf8');
  const toc = parseSummary(summary);
  await ensureDir(publicManualDir);
  await fs.writeFile(path.join(publicManualDir, 'toc.json'), JSON.stringify(toc, null, 2), 'utf8');
}

async function main() {
  await copyDir(docsSrcDir, publicManualDir);

  try {
    await copyDir(docsScreensDir, publicScreensDir);
  } catch (err) {
    if (err?.code !== 'ENOENT') {
      throw err;
    }
  }

  await writeTocJson();
}

main().catch((err) => {
  console.error('[docs:sync] Failed to sync manual:', err);
  process.exit(1);
});
