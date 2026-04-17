###
# This is a TEMPLATE for the Homebrew formula. The actual formula lives in
# the separate tap repository at https://github.com/glebmatz/homebrew-cascade.
#
# The release GitHub Action auto-generates this file on every tag push and
# commits it to the tap. You normally shouldn't edit this file by hand — edit
# `.github/workflows/release.yml` instead if you need to change the template.
###
class Cascade < Formula
  desc "Terminal rhythm game with automatic beatmap generation"
  homepage "https://github.com/glebmatz/cascade"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/glebmatz/cascade/releases/download/v0.1.0/cascade-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_ME_DARWIN_ARM64"
    else
      url "https://github.com/glebmatz/cascade/releases/download/v0.1.0/cascade-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_ME_DARWIN_X64"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/glebmatz/cascade/releases/download/v0.1.0/cascade-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_ME_LINUX_ARM64"
    else
      url "https://github.com/glebmatz/cascade/releases/download/v0.1.0/cascade-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_ME_LINUX_X64"
    end
  end

  def install
    bin.install "cascade"
  end

  test do
    assert_match "Cascade", shell_output("#{bin}/cascade help")
  end
end
