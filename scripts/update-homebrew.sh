#!/bin/bash

# This script updates the Homebrew Cask definition in the tap repository.
# Expected environment variables:
# HOMEBREW_TAP_TOKEN: GitHub Personal Access Token with repo scope

set -e

TAP_REPO="blue1st/homebrew-taps"
CASK_NAME="kindle-glean"
PACKAGE_JSON="package.json"

# Get version from package.json
VERSION=$(node -p "require('./$PACKAGE_JSON').version")
echo "Updating Homebrew Cask to version $VERSION"

# We expect DMGs to be downloaded into current dir or subdirectories
DMG_ARM=$(find . -name "*_aarch64.dmg" | head -n 1)
DMG_X64=$(find . -name "*_x64.dmg" -o -name "*_x86_64.dmg" | head -n 1)

if [ -z "$DMG_ARM" ] && [ -z "$DMG_X64" ]; then
  echo "Error: Could not find any DMG files"
  exit 1
fi

# Fallback if one architecture DMG is not generated
DMG_ARM=${DMG_ARM:-$DMG_X64}
DMG_X64=${DMG_X64:-$DMG_ARM}

SHA256_ARM=$(shasum -a 256 "$DMG_ARM" | awk '{print $1}')
SHA256_X64=$(shasum -a 256 "$DMG_X64" | awk '{print $1}')

echo "ARM SHA256: $SHA256_ARM"
echo "X64 SHA256: $SHA256_X64"

# Clone the tap repository
TMP_DIR=$(mktemp -d)
git clone "https://x-access-token:${HOMEBREW_TAP_TOKEN}@github.com/${TAP_REPO}.git" "$TMP_DIR"

# Ensure Casks directory exists
mkdir -p "$TMP_DIR/Casks"

CASK_FILE="$TMP_DIR/Casks/${CASK_NAME}.rb"

# Create or update the Cask file
cat <<EOF > "$CASK_FILE"
cask "${CASK_NAME}" do
  arch arm: "aarch64", intel: "x64"

  version "${VERSION}"
  sha256 arm:   "${SHA256_ARM}",
         intel: "${SHA256_X64}"

  url "https://github.com/blue1st/kindle-glean/releases/download/v#{version}/Kindle.Glean_#{version}_#{arch}.dmg"
  name "Kindle Glean"
  desc "Extract Kindle highlights, notes, and vocabulary to local Markdown"
  homepage "https://github.com/blue1st/kindle-glean"

  app "Kindle Glean.app"

  postflight do
    system_command "xattr",
                   args: ["-cr", "#{appdir}/Kindle Glean.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.kindleglean.app",
    "~/Library/Preferences/com.kindleglean.app.plist",
    "~/Library/Saved Application State/com.kindleglean.app.savedState",
    "~/Library/WebKit/com.kindleglean.app",
  ]
end
EOF

# Commit and push
cd "$TMP_DIR"
git config user.name "github-actions[bot]"
git config user.email "github-actions[bot]@users.noreply.github.com"
git add "Casks/${CASK_NAME}.rb"
git commit -m "Update ${CASK_NAME} to v${VERSION}" || echo "No changes to commit"
git push origin main

echo "Homebrew tap updated successfully!"
