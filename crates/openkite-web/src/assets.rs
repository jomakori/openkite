//! The hydration bundle, versioned by its own bytes.
//!
//! wasm-bindgen's glue and the `.wasm` it imports are one unit: the glue resolves
//! its sibling module relative to `import.meta.url`. The SSR document imported
//! both from an unversioned `/openkite-web-client.js` and the host served them
//! with no cache directive, so a returning browser was free to pair the new HTML
//! it just fetched with the wasm it had kept, and hydrate new markup with old
//! code. Nothing in the page said which build the URL belonged to.
//!
//! The fix is content addressing. The host hashes the two files it serves, the
//! document imports them from `/assets/bundle/{version}/…`, and the route
//! answers a version only when it is the one this process computed. A page and
//! its wasm therefore come from the same build, or the module does not load at
//! all — never a mix of the two.
//!
//! The cache policy follows from the same fact: a hashed URL names bytes that
//! cannot change, so it may be kept forever ([`IMMUTABLE`]); the document names
//! the current build, so it may not be reused ([`DOCUMENT`]).

use std::path::Path;

use sha2::{Digest, Sha256};

/// The glue wasm-bindgen writes for the client binary.
pub const CLIENT_JS: &str = "openkite-web-client.js";

/// The module that glue imports, resolved as its sibling.
pub const CLIENT_WASM: &str = "openkite-web-client_bg.wasm";

/// Path prefix the versioned pair is served from. Under `/assets/` because that
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

/// The hydration pair this process serves, named by the hash of its bytes.
#[derive(Debug, Clone)]
pub struct Bundle {
    version: String,
}

impl Bundle {
    /// Hash the two files `web_root` carries.
    ///
    /// A missing file hashes as empty rather than failing the boot: the console
    /// then renders without hydrating, which is what a host with no bundle
    /// already did — the import 404s instead of the process refusing to start.
    pub fn load(web_root: &Path) -> Self {
        Self {
            version: digest(web_root),
        }
    }

    /// The build's identity: the hash of the glue and the module together.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The URL the document imports.
    ///
    /// The glue resolves `openkite-web-client_bg.wasm` against this URL, so the
    /// module lands under the same version without the host rewriting it.
    pub fn client_url(&self) -> String {
        format!("{MOUNT}/{}/{CLIENT_JS}", self.version)
    }

    /// The file a request names, when the version is this build's and the file
    /// is one this mount carries. `None` is a 404: bytes from another build are
    /// never served under this build's name.
    pub fn resolve(&self, version: &str, file: &str) -> Option<&'static str> {
        if version != self.version {
            return None;
        }
        match file {
            CLIENT_JS => Some(CLIENT_JS),
            CLIENT_WASM => Some(CLIENT_WASM),
            _ => None,
        }
    }

    /// The type a browser has to be told for the file to be usable: the module
    /// only compiles as `application/wasm`.
    pub fn content_type(file: &str) -> &'static str {
        match file {
            CLIENT_WASM => "application/wasm",
            _ => "text/javascript; charset=utf-8",
        }
    }
}

/// SHA-256 over both files, each length-prefixed so a byte moving between them
/// changes the hash, truncated to 16 hex characters.
fn digest(web_root: &Path) -> String {
    let mut hasher = Sha256::new();
    for name in [CLIENT_JS, CLIENT_WASM] {
        let bytes = std::fs::read(web_root.join(name)).unwrap_or_default();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
