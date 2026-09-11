// Fails when the committed vendored React bundle drifts from a fresh build.
//
// Run after `npm run build && npm run bundle-size` (the documented rebuild
// sequence): the working tree then holds exactly what should be committed, so
// any diff against the index means the vendored copy is stale.
import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = resolve(here, '..')

const pkg = JSON.parse(readFileSync(join(webRoot, 'package.json'), 'utf8'))
const bundleDir = resolve(webRoot, pkg.openkite.vendoredBundleDir)

const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], {
  cwd: webRoot,
  encoding: 'utf8',
}).trim()
const bundleDirRel = relative(repoRoot, bundleDir).split(sep).join('/')

if (!existsSync(bundleDir)) {
  console.error(`Vendored bundle directory is missing: ${bundleDirRel}`)
  process.exit(1)
}

function git(args) {
  return execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' })
}

let stale = false
try {
  execFileSync('git', ['diff', '--exit-code', '--', bundleDirRel], {
    cwd: repoRoot,
    stdio: 'ignore',
  })
} catch {
  stale = true
}

const untracked = git(['ls-files', '--others', '--exclude-standard', '--', bundleDirRel]).trim()
if (untracked) {
  stale = true
  console.error(`\nUntracked file(s) the build emitted:\n${untracked}`)
}

if (!stale) {
  console.log(`Vendored bundle is fresh: ${bundleDirRel}`)
  process.exit(0)
}

const stat = git(['diff', '--stat', '--', bundleDirRel]).trim()
if (stat) {
  console.error(`\n${stat}`)
}

console.error(
  [
    '',
    '============================================================',
    'Vendored React bundle is STALE.',
    '============================================================',
    'The freshly built bundle does not match what is committed under:',
    `  ${bundleDirRel}`,
    '',
    'The Rust host include_str!s these files, so a stale copy ships',
    'without CI noticing.',
    '',
    'Fix: rebuild and commit the vendored output:',
    '',
    '  cd web && npm ci && npm run build && npm run bundle-size',
    '',
    `then commit the changes under ${bundleDirRel}.`,
    '============================================================',
  ].join('\n'),
)
process.exit(1)
