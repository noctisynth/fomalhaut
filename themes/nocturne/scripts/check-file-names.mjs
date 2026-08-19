import { readdir } from "node:fs/promises";
import path from "node:path";

const ignoredDirectories = new Set(["dist", "node_modules"]);
const fixedNames = new Set([
  ".gitignore",
  "CHANGELOG.md",
  "biome.json",
  "components.json",
  "index.html",
  "package.json",
  "theme.toml",
  "tsconfig.json",
]);
const kebabCaseName = /^[a-z0-9]+(?:-[a-z0-9]+)*(?:\.[a-z0-9]+)*$/;
const failures = [];

async function inspect(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    const name = entry.name;
    if (ignoredDirectories.has(name)) {
      continue;
    }
    if (!fixedNames.has(name) && !kebabCaseName.test(name)) {
      failures.push(path.relative(process.cwd(), absolute));
    }
    if (entry.isDirectory()) {
      await inspect(absolute);
    }
  }
}

await inspect(process.cwd());

if (failures.length > 0) {
  console.error(`Non-kebab-case project paths:\n${failures.join("\n")}`);
  process.exit(1);
}
