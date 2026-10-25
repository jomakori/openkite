//! The hydration bundle, versioned by its own bytes.
//!
//! wasm-bindgen does not emit a single file. The glue `openkite-web-client.js`
//! imports its `.wasm` sibling *and* a tree of `snippets/` modules, all by
//! relative path, resolved against `import.meta.url`. The SSR document imported
//! the glue from an unversioned `/openkite-web-client.js` and the host served
//! everything with no cache directive at all, so a returning browser was free to
//! pair new HTML with the wasm and snippets it had kept — hydrate new markup with
//! old code — and nothing in the page said which build the URL belonged to.
//!
//! The fix is content addressing. The host hashes the whole output set, the
//! document imports it from `/assets/bundle/{version}/`, and the route answers a
//! version only when it is the one this process computed. Because the relative
//! imports live under the versioned directory, a page and every module it loads
//! come from the same build or the module does not load at all — never a mix.
//!
//! The cache policy follows from the same fact: a hashed URL names bytes that
//! cannot change, so it may be kept forever ([`IMMUTABLE`]); the document names
//! the current build, so it may not be reused ([`DOCUMENT`]).

use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

/// The glue wasm-bindgen writes for the client binary.
pub const CLIENT_JS: &str = "openkite-web-client.js";

/// The module that glue imports, resolved as its sibling.
pub const CLIENT_WASM: &str = "openkite-web-client_bg.wasm";

/// The tree the glue imports by relative path (`./snippets/...`).
pub const SNIPPETS: &str = "snippets";

/// Path prefix the versioned set is served from. Under `/assets/` because that
/// is where the crate's other static bytes live, and away from `/assets/fonts/`
/// so the typeface route keeps its own shape.
pub const MOUNT: &str = "/assets/bundle";

/// A hashed asset is one build's own bytes: it can never be wrong later, so a
/// browser or an edge cache may keep it without asking again.
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// The document names the current build, so it has to be re-read on every
/// navigation — a cached document is a document that can be paired with a wasm
/// from another build.
pub const DOCUMENT: &str = "no-store";

/// The hydration set this process serves, named by the hash of its bytes.
#[derive(Debug, Clone)]
pub struct Bundle {
    version: String,
    /// Always [`CLIENT_JS`]: the document names this one file, and the rest of
    /// the set is reached from it by relative import.
    entry: String,
}

impl Bundle {
    /// Hash the hydration set `web_root` carries.
    ///
    /// An empty set hashes as empty rather than failing the boot: the console
    /// then renders without hydrating, which is what a host with no bundle
    /// already did — the import 404s instead of the process refusing to start.
    pub fn load(web_root: &Path) -> Self {
        Self {
            version: digest(&files(web_root)),
            entry: CLIENT_JS.to_string(),
        }
    }

    /// The build's identity: the hash of the glue, the module and the snippets
    /// together.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The URL the document imports.
    ///
    /// The glue resolves its wasm and its `./snippets/…` imports against this
    /// URL, so every file of the set lands under the same version without the
    /// host rewriting the glue.
    pub fn client_url(&self) -> String {
        format!("{MOUNT}/{}/{}", self.version, self.entry)
    }

    /// The file a request names, when the version is this build's and the path is
    /// one the set actually carries.
    ///
    /// `None` is a 404: bytes from another build are never served under this
    /// build's name, and a path outside the set — a `..` segment, an absolute
    /// path, anything beside the glue and its snippets — is not a file of the
    /// bundle no matter what the disk holds next to it.
    pub fn resolve(&self, version: &str, rest: &str) -> Option<PathBuf> {
        if version != self.version {
            return None;
        }
        let path = sanitize(rest)?;
        let in_set = match path.components().next()? {
            Component::Normal(name) => name == CLIENT_JS || name == CLIENT_WASM || name == SNIPPETS,
            _ => false,
        };
        in_set.then_some(path)
    }

    /// The type a browser has to be told for the file to be usable: the module
    /// only compiles as `application/wasm`, and the snippets only as modules.
    pub fn content_type(path: &Path) -> &'static str {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("wasm") => "application/wasm",
            Some("js" | "mjs") => "text/javascript; charset=utf-8",
            _ => "application/octet-stream",
        }
    }
}

/// A relative path with no `.`/`..` and no root — the only shape the mount
/// answers. Anything else could name a file the bundle does not own.
fn sanitize(rest: &str) -> Option<PathBuf> {
    if rest.is_empty() {
        return None;
    }
    let path = Path::new(rest);
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            _ => return None,
        }
    }
    Some(clean)
}

/// The hydration set under `web_root`: the glue, the module, and every file in
/// `snippets/`, as `(relative path, bytes)` sorted by path so the hash is stable
/// across filesystems.
fn files(web_root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut set = Vec::new();
    for name in [CLIENT_JS, CLIENT_WASM] {
        let bytes = std::fs::read(web_root.join(name)).unwrap_or_default();
        if !bytes.is_empty() {
            set.push((name.to_string(), bytes));
        }
    }
    collect(&web_root.join(SNIPPETS), SNIPPETS, &mut set);
    set.sort_by(|a, b| a.0.cmp(&b.0));
    set
}

/// Walk `dir`, appending regular files as paths relative to `web_root`.
fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let rel = format!("{prefix}/{name}");
        if path.is_dir() {
            collect(&path, &rel, out);
        } else if let Ok(bytes) = std::fs::read(&path) {
            out.push((rel, bytes));
        }
    }
}

/// SHA-256 over the whole set, each path and its length fed in before its bytes
/// so a rename or a byte moving between files changes the hash. Truncated to 16
/// hex characters: long enough that two builds never collide by accident, short
/// enough to read in a log.
fn digest(files: &[(String, Vec<u8>)]) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
