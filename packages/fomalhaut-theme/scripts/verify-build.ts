import path from "node:path";

const dist = path.join(process.cwd(), "dist");
const requiredFiles = ["index.html", "theme.toml"];
const maximumAssetBytes = 8 * 1024 * 1024;
const failures: string[] = [];
const networkApiPattern =
  /\b(?:fetch|WebSocket|EventSource|XMLHttpRequest)\s*\(|\bnavigator\.sendBeacon\s*\(/;

for await (const entry of new Bun.Glob("**/*.{ts,tsx}").scan({
  cwd: path.join(process.cwd(), "src"),
  onlyFiles: true,
})) {
  const source = await Bun.file(path.join(process.cwd(), "src", entry)).text();
  if (networkApiPattern.test(source)) {
    failures.push(`src/${entry} calls a network API`);
  }
}

for (const name of requiredFiles) {
  if (!(await Bun.file(path.join(dist, name)).exists())) {
    failures.push(`missing ${name}`);
  }
}

const html = await Bun.file(path.join(dist, "index.html")).text();
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

for await (const entry of new Bun.Glob("**/*").scan({
  cwd: dist,
  onlyFiles: true,
})) {
  const file = Bun.file(path.join(dist, entry));
  if (file.size > maximumAssetBytes) {
    failures.push(`${entry} exceeds the host asset limit`);
  }
  if (entry.endsWith(".css")) {
    const css = await file.text();
    if (/url\(["']?https?:/i.test(css)) {
      failures.push(`${entry} contains a remote CSS resource`);
    }
  }
  if (entry.endsWith(".js")) {
    const javascript = await file.text();
    if (javascript.includes("FOMALHAUT_DEVELOPMENT_TRANSPORT")) {
      failures.push(`${entry} contains the development transport`);
    }
  }
}

if (failures.length > 0) {
  console.error(`Invalid theme build:\n${failures.join("\n")}`);
  process.exit(1);
}
