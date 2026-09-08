cask "openkite" do
  version "0.26.4"
  sha256 "4c7cb0206a135a177578aae9fd0ae4d249070caf019ce7d0d378156d140c4a6e"

  url "https://github.com/jomakori/openkite/releases/download/v#{version}/openkite_#{version}_macos_arm64.dmg",
      verified: "github.com/jomakori/openkite/releases/download/"
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
