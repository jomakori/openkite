// Writes the released version into the workspace manifest and the lockfile.
// The lockfile is edited directly: resolving it would need the registry, and the
// only entries that change are our own crates.
import { readFileSync, writeFileSync, existsSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error(`expected a semver version, got ${version ?? "nothing"}`);
  process.exit(1);
}

const prefix = "openkite";

const manifest = "Cargo.toml";
const source = readFileSync(manifest, "utf8");
const bumped = source.replace(
  /(\[workspace\.package\][^[]*?\nversion\s*=\s*)"[^"]*"/,
  `$1"${version}"`,
);
if (bumped === source) {
  console.error("no [workspace.package] version line to rewrite");
  process.exit(1);
}
writeFileSync(manifest, bumped);

const lock = "Cargo.lock";
if (existsSync(lock)) {
  let current = null;
  let touched = 0;
  const rewritten = readFileSync(lock, "utf8")
    .split("\n")
    .map((line) => {
      const name = line.match(/^name = "([^"]+)"$/);
      if (name) {
        current = name[1];
        return line;
      }
      if (current?.startsWith(prefix) && /^version = "[^"]*"$/.test(line)) {
        touched += 1;
        current = null;
        return `version = "${version}"`;
      }
      return line;
    })
    .join("\n");
  if (touched === 0) {
    console.error(`no ${prefix}* package in ${lock} to rewrite`);
    process.exit(1);
  }
  writeFileSync(lock, rewritten);
  console.log(`Cargo.toml + ${lock} parked at ${version} (${touched} crates)`);
} else {
  console.log(`Cargo.toml parked at ${version}; no ${lock} present`);
}
