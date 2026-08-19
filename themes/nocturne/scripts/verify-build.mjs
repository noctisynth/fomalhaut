import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";

const dist = path.join(process.cwd(), "dist");
const requiredFiles = ["index.html", "theme.toml"];
const maximumAssetBytes = 8 * 1024 * 1024;
const failures = [];
let disablesPageTextSelection = false;
let enablesEditableTextSelection = false;
const networkApiPattern =
  /\b(?:fetch|WebSocket|EventSource|XMLHttpRequest)\s*\(|\bnavigator\.sendBeacon\s*\(/;
const costlyCompositingClassPattern =
  /\b(?:backdrop-blur(?:-[a-z0-9-]+)?|blur-(?:2xl|3xl))\b/;

async function* files(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      yield* files(absolute);
    } else if (entry.isFile()) {
      yield absolute;
    }
  }
}

async function isFile(file) {
  try {
    return (await stat(file)).isFile();
  } catch {
    return false;
  }
}

const sourceRoot = path.join(process.cwd(), "src");
for await (const file of files(sourceRoot)) {
  if (!/\.tsx?$/.test(file)) {
    continue;
  }
  const entry = path.relative(sourceRoot, file);
  const source = await readFile(file, "utf8");
  if (networkApiPattern.test(source)) {
    failures.push(`src/${entry} calls a network API`);
  }
  if (costlyCompositingClassPattern.test(source)) {
    failures.push(`src/${entry} enables a prohibited compositing blur`);
  }
}

for (const name of requiredFiles) {
  if (!(await isFile(path.join(dist, name)))) {
    failures.push(`missing ${name}`);
  }
}

const manifest = await readFile(path.join(dist, "theme.toml"), "utf8");
if (!/^id\s*=\s*"nocturne"\s*$/m.test(manifest)) {
  failures.push("theme.toml does not declare the stable nocturne ID");
}

const html = await readFile(path.join(dist, "index.html"), "utf8");
if (/<script(?![^>]*\bsrc=)[^>]*>/i.test(html)) {
  failures.push("index.html contains an inline script");
}
if (/<style\b/i.test(html) || /\sstyle=/i.test(html)) {
  failures.push("index.html contains inline styles");
}
if (/\b(?:src|href)=["']\//i.test(html)) {
  failures.push("index.html contains an absolute asset URL");
}
if (/\b(?:src|href)=["']https?:/i.test(html)) {
  failures.push("index.html contains a remote asset URL");
}
if (/<form[^>]+\baction=/i.test(html)) {
  failures.push("index.html enables form navigation");
}

for await (const file of files(dist)) {
  const entry = path.relative(dist, file);
  if ((await stat(file)).size > maximumAssetBytes) {
    failures.push(`${entry} exceeds the host asset limit`);
  }
  if (entry.endsWith(".css")) {
    const css = await readFile(file, "utf8");
    disablesPageTextSelection ||=
      /body\s*\{[^}]*[;{]user-select\s*:\s*none/.test(css);
    enablesEditableTextSelection ||=
      /input\s*,\s*textarea\s*,\s*\[contenteditable\]:not\(\[contenteditable=(?:"false"|false)\]\)\s*\{[^}]*[;{]user-select\s*:\s*text/.test(
        css,
      );
    if (/url\(["']?https?:/i.test(css)) {
      failures.push(`${entry} contains a remote CSS resource`);
    }
    if (/backdrop-filter\s*:|\.blur-(?:2xl|3xl)\{/i.test(css)) {
      failures.push(`${entry} contains a prohibited compositing blur`);
    }
  }
  if (entry.endsWith(".js")) {
    const javascript = await readFile(file, "utf8");
    if (javascript.includes("FOMALHAUT_DEVELOPMENT_TRANSPORT")) {
      failures.push(`${entry} contains the development transport`);
    }
  }
}

if (!disablesPageTextSelection) {
  failures.push("CSS does not disable selection for ordinary page text");
}
if (!enablesEditableTextSelection) {
  failures.push("CSS does not restore selection for editable text");
}

if (failures.length > 0) {
  console.error(`Invalid theme build:\n${failures.join("\n")}`);
  process.exit(1);
}
