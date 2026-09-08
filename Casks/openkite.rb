cask "openkite" do
  version "0.26.4"

  on_arm do
    url "https://github.com/jomakori/openkite/releases/download/v#{version}/openkite_#{version}_macos_arm64.dmg",
        verified: "github.com/jomakori/openkite/releases/download/"
    sha256 "4c7cb0206a135a177578aae9fd0ae4d249070caf019ce7d0d378156d140c4a6e"
  end
  on_intel do
    url "https://github.com/jomakori/openkite/releases/download/v#{version}/openkite_#{version}_macos_amd64.dmg",
        verified: "github.com/jomakori/openkite/releases/download/"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
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
