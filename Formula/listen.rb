# Updated by .github/workflows/release.yml on every tagged release.
class Listen < Formula
  desc "Terminal app for intensive English listening practice with spaced-repetition review"
  homepage "https://github.com/wine-fall/language-learning"
  version "0.1.0"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "151cfb0cddd8f33501e1892ecfef6f5a4b752b9fdf3dccd636823b0b6f6a555e"
  end

  on_intel do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "22c8606d62f49c38706a19425745044489c20619e0f230587c320da94f162e57"
  end

  def install
    bin.install "listen"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/listen --version")
  end
end
