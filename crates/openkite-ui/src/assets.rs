//! The vendored IBM Plex faces, and the two ways a host puts them in front of
//! a renderer.
//!
//! `assets/main.css` declares every face by the absolute URL the browser host
//! serves it from ([`FONT_ROUTE`]): a served page fetches each file once and
//! caches it. A webview has no origin to serve them from, so the desktop host
//! inlines [`embedded_css`] instead — the same stylesheet with every face URL
//! replaced by a `data:` URI, which costs the ~4/3 base64 premium over the raw
//! WOFF2 and no request at all.

use base64::Engine as _;

/// Where the browser host mounts the faces. `assets/main.css` names each one
/// under this prefix, so the route and the stylesheet have to agree.
pub const FONT_ROUTE: &str = "/assets/fonts/";

/// One vendored WOFF2 face and the `@font-face` descriptors it ships with.
pub struct Face {
    /// File name under `assets/fonts/`, and the last segment of its URL.
    pub file: &'static str,
    /// The `font-family` the stylesheet declares for it.
    pub family: &'static str,
    /// The `font-weight` the stylesheet declares for it.
    pub weight: u16,
    /// The WOFF2 bytes, embedded at compile time.
    pub bytes: &'static [u8],
}

/// Every face the shell stylesheet declares: IBM Plex Sans 400/500/600/700 and
/// IBM Plex Mono 400/500/600, each IBM's Latin1 web subset.
pub const FACES: &[Face] = &[
    Face {
        file: "ibm-plex-sans-400.woff2",
        family: "IBM Plex Sans",
        weight: 400,
        bytes: include_bytes!("../assets/fonts/ibm-plex-sans-400.woff2"),
    },
    Face {
        file: "ibm-plex-sans-500.woff2",
        family: "IBM Plex Sans",
        weight: 500,
        bytes: include_bytes!("../assets/fonts/ibm-plex-sans-500.woff2"),
    },
    Face {
        file: "ibm-plex-sans-600.woff2",
        family: "IBM Plex Sans",
        weight: 600,
        bytes: include_bytes!("../assets/fonts/ibm-plex-sans-600.woff2"),
    },
    Face {
        file: "ibm-plex-sans-700.woff2",
        family: "IBM Plex Sans",
        weight: 700,
        bytes: include_bytes!("../assets/fonts/ibm-plex-sans-700.woff2"),
    },
    Face {
        file: "ibm-plex-mono-400.woff2",
        family: "IBM Plex Mono",
        weight: 400,
        bytes: include_bytes!("../assets/fonts/ibm-plex-mono-400.woff2"),
    },
    Face {
        file: "ibm-plex-mono-500.woff2",
        family: "IBM Plex Mono",
        weight: 500,
        bytes: include_bytes!("../assets/fonts/ibm-plex-mono-500.woff2"),
    },
    Face {
        file: "ibm-plex-mono-600.woff2",
        family: "IBM Plex Mono",
        weight: 600,
        bytes: include_bytes!("../assets/fonts/ibm-plex-mono-600.woff2"),
    },
];

/// The face served as `file`, or `None` when it is not vendored.
pub fn face_by_file(file: &str) -> Option<&'static Face> {
    FACES.iter().find(|face| face.file == file)
}

/// The URL the stylesheet names for one face.
pub fn src_url(face: &Face) -> String {
    format!("{FONT_ROUTE}{}", face.file)
}

/// The `data:` URI that stands in for one face in an embedded stylesheet.
pub fn data_uri(face: &Face) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(face.bytes);
    format!("data:font/woff2;base64,{encoded}")
}

/// Total payload of the vendored faces: what the browser host serves over the
/// network, and what the desktop host carries inside the binary.
pub fn payload_bytes() -> usize {
    FACES.iter().map(|face| face.bytes.len()).sum()
}

/// [`crate::MAIN_CSS`] with every face URL replaced by its data URI, so a
/// renderer with no origin to fetch from still paints the design's type.
pub fn embedded_css() -> String {
    let mut css = crate::MAIN_CSS.to_string();
    for face in FACES {
        css = css.replace(
            &format!("url(\"{}\")", src_url(face)),
            &format!("url(\"{}\")", data_uri(face)),
        );
    }
    css
}
