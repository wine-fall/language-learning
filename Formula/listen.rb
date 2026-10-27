# Updated by .github/workflows/release.yml on every tagged release.
class Listen < Formula
  desc "Terminal app for intensive English listening practice with spaced-repetition review"
  homepage "https://github.com/wine-fall/language-learning"
  version "0.1.1"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "3fc35259f14a35fed766d6576467e3a9bec2691941416025c73b9b82d0e210ae"
  end

  on_intel do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "b845ddbd19f30ad8d818ffe2f281dea6cc0183f47517c8e03ab870abf1173a4e"
  end

  def install
    bin.install "listen"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/listen --version")
  end
end
