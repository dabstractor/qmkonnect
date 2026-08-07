# frozen_string_literal: true
#
# Homebrew Cask for QMKonnect — distributed via a custom tap
#   brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
#   brew install --cask qmkonnect
# until notarization qualifies it for the official homebrew-cask repo (PRD §12).
#
# CI (.github/workflows/release.yml) patches `version` and `sha256` on each tagged
# release and pushes this file to the tap repo (architecture/external_deps.md §"Automation").
# The `sha256 :no_check` below is the template placeholder CI overwrites with the
# real `shasum -a 256` of QMKonnect-<version>-macos.dmg.
#
# Validate locally:  brew audit --cask --new-cask ./qmkonnect.rb   (DSL/token/order only)
#                    ruby -c qmkonnect.rb                            (syntax, any host)

cask "qmkonnect" do
  # CI replaces both fields on each tagged release.
  version "0.2.8"
  sha256 :no_check   # template placeholder — CI overwrites with the release DMG's real hash

  url "https://github.com/dabstractor/qmkonnect/releases/download/v#{version}/QMKonnect-#{version}-macos.dmg",
      verified: "github.com/dabstractor/qmkonnect/"

  name "QMKonnect"
  desc "Cross-platform window activity notifier for QMK keyboards"
  homepage "https://github.com/dabstractor/qmkonnect"

  livecheck do
    url "https://github.com/dabstractor/qmkonnect/releases/latest"
    regex(/^v?(\d+(?:\.\d+)+)$/i)
    strategy :header
  end

  app "QMKonnect.app"

  zap trash: [
    "~/Library/Application Support/QMKonnect/",
  ]

  caveats <<~EOS
    QMKonnect needs Screen Recording permission to read window titles. On first
    launch, grant it at System Settings → Privacy & Security → Screen Recording
    (the app runs without it, but sends only app names, not window titles).

    Discover the exact class/title strings for your rules.toml with:
        qmkonnect --show-window-info

    The released DMG is ad-hoc signed (not yet notarized). If macOS Gatekeeper
    blocks the first launch, either right-click the app → "Open", clear the
    quarantine attribute:
        xattr -dr com.apple.quarantine /Applications/QMKonnect.app
    or install with:
        brew install --cask --no-quarantine qmkonnect
  EOS
end