use dioxus::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();
    dioxus::launch(App);
}

/// Root component — sidebar shell + placeholder content.
/// Core routes and plugin route merging land in the sidebar/router ticket (OKT-7).
fn App() -> Element {
    rsx! {
        div { class: "app-shell",
            aside { class: "sidebar",
                h1 { class: "brand", "OpenKite" }
                span { class: "tagline", "Kubernetes from above." }
                nav { class: "nav" }
            }
            main { class: "content",
                h2 { "No cluster connected" }
                p { "Connect a kubeconfig context to begin." }
            }
        }
    }
}
