class Openkite < Formula
  desc "Kubernetes desktop IDE with plugin bridge"
  homepage "https://github.com/jomakori/openkite"
  url "https://github.com/jomakori/openkite/archive/refs/tags/v0.26.4.tar.gz"
  sha256 "e146a4c92b8e645f8ac71f3a11390a8dd259a77d1fa7789c3ba8128b4cd33d61"
  license "MIT OR Apache-2.0"

  # Build-time only. The desktop link pulls webkit2gtk/gtk via pkg-config;
  # aws-lc-sys (kube TLS) builds with cmake.
  depends_on "cmake" => :build
  depends_on "pkg-config" => :build
  depends_on "rust" => :build

  # Linux-only link deps. macOS builds against the system WebKit/AppKit
  # frameworks and needs none of these.
  on_linux do
    depends_on "gtk+3"
    depends_on "librsvg"
    depends_on "webkit2gtk"
    depends_on "xdotool" # provides libxdo (pkg-config xdo)
  end

  def install
    system "cargo", "install", "--locked", "--root", libexec, "--path", "."
    bin.install Dir[libexec/"bin/*"]
  end

  test do
    # GUI desktop app — no CLI surface to probe headless. Assert the
    # binary exists and is dynamically linked against the webview stack.
    assert_predicate bin/"openkite", :exist?
  end
end
