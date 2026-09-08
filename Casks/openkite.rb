cask "openkite" do
  version "0.27.0"

  on_arm do
    url "https://github.com/jomakori/openkite/releases/download/v#{version}/openkite_#{version}_macos_arm64.dmg",
        verified: "github.com/jomakori/openkite/releases/download/"
    sha256 "5d81a1a8012077d98ce648198db1a55abaa030cfb7eedd115ea52e222cf20d49"
  end
  on_intel do
    url "https://github.com/jomakori/openkite/releases/download/v#{version}/openkite_#{version}_macos_amd64.dmg",
        verified: "github.com/jomakori/openkite/releases/download/"
    sha256 "0f174720c11de8a8575ac9471920fe7f88004b484d07ce8bbadc517f4d8a19b9"
  end

  name "OpenKite"
  desc "Kubernetes desktop IDE with plugin bridge"
  homepage "https://github.com/jomakori/openkite"

  app "OpenKite.app"

  zap trash: [
    "~/.openkite",
    "~/Library/Application Support/com.openkite.app",
    "~/Library/Caches/com.openkite.app",
  ]
end
