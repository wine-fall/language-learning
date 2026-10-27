# Updated by .github/workflows/release.yml on every tagged release.
class Listen < Formula
  desc "Terminal app for intensive English listening practice with spaced-repetition review"
  homepage "https://github.com/wine-fall/language-learning"
  version "0.1.4"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "3f6920f562333caac415f55155c360a0befe06833faa261f0e7873e8ceb2ec11"
  end

  on_intel do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "211cd0859eb89110c87bd434811730ac130f11cc913939bdf5dfc39185146315"
  end

  def install
    bin.install "listen"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/listen --version")
  end
end
