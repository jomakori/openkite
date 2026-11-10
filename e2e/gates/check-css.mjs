#!/usr/bin/env node
// Gate G6, static half: the stylesheet carries no literal colour outside its
// token block, and the reduced-motion block zeroes transitions.
//
// A literal colour in a surface rule is a colour that ignores the theme: the
// token block is the one place a hex may appear, because that is where a hex is
// *defined* rather than repeated. Reads the sheet as text, so it needs no
// browser and fails in a second.
//
// Usage: node e2e/gates/check-css.mjs [path/to/main.css]
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const sheetPath = path.resolve(process.argv[2] ?? path.join(scriptDir, '../../crates/openkite-ui/assets/main.css'));

const HEX = /#[0-9a-fA-F]{3,8}\b/g;
const failures = [];

let source;
try {
  source = readFileSync(sheetPath, 'utf8');
} catch (error) {
  console.error(`FAIL: cannot read ${sheetPath}: ${error.message}`);
  process.exit(1);
}

const lines = source.split('\n');
// The declaration a continuation line belongs to: a value may wrap, and a hex
// on the wrapped line is still that declaration's colour.
let currentProp = '';
let inBlock = false;

for (const [index, line] of lines.entries()) {
  const lineNo = index + 1;
  const trimmed = line.trim();
  if (trimmed.startsWith('/*') || trimmed.startsWith('*')) {
    if (trimmed.endsWith('*/')) currentProp = inBlock ? currentProp : '';
    else currentProp = trimmed.endsWith('*/') ? '' : currentProp;
    continue;
  }
  if (trimmed === '}' || trimmed.startsWith('}')) {
    inBlock = false;
    currentProp = '';
  }
  if (trimmed.endsWith('{')) {
    inBlock = true;
    currentProp = '';
  }
  const colon = trimmed.indexOf(':');
  if (trimmed.endsWith(';') && colon > 0) {
    currentProp = trimmed.slice(0, colon).trim();
    if (trimmed.endsWith('}')) {
      inBlock = false;
      currentProp = '';
    }
  }
  const matches = [...line.matchAll(HEX)];
  if (matches.length === 0) continue;
  // A token definition is the one place a literal colour belongs.
  if (currentProp.startsWith('--')) continue;
  for (const match of matches) {
    failures.push(`${path.relative(process.cwd(), sheetPath)}:${lineNo}: literal ${match[0]} in \`${trimmed}\` (declare a token in :root and use var())`);
  }
}

// The reduced-motion contract: the block exists, and it zeroes both duration
// properties for every element, including pseudo-elements.
const reducedMotion = source.match(/@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{([\s\S]*?)\n\}/);
if (!reducedMotion) {
  failures.push(`${sheetPath}: no @media (prefers-reduced-motion: reduce) block`);
} else {
  const body = reducedMotion[1];
  for (const property of ['transition-duration', 'animation-duration']) {
    const value = body.match(new RegExp(`${property}\\s*:\\s*([^;!]+)`));
    if (!value) {
      failures.push(`prefers-reduced-motion block does not set ${property}`);
      continue;
    }
    const parsed = Number.parseFloat(value[1]);
    if (!Number.isFinite(parsed) || parsed > 0.001 || Number.isNaN(parsed)) {
      failures.push(`prefers-reduced-motion sets ${property} to \`${value[1].trim()}\`; it must be zero (or the 0.001ms nudge)`);
    }
  }
  if (!/\*\s*,\s*\*::before\s*,\s*\*::after/.test(body)) {
    failures.push('prefers-reduced-motion does not target `*, *::before, *::after`');
  }
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`FAIL: ${failure}`);
  console.error(`FAIL: G6 static — ${failures.length} stylesheet failure(s)`);
  process.exit(1);
}
console.log(`PASS: G6 static — no literal colour outside the token block, reduced motion zeroes transitions (${lines.length} lines)`);
