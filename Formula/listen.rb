# Updated by .github/workflows/release.yml on every tagged release.
class Listen < Formula
  desc "Terminal app for intensive English listening practice with spaced-repetition review"
  homepage "https://github.com/wine-fall/language-learning"
  version "0.1.3"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "a189f81ab5b29dfa87af1777e231c7b8e838caeb1680bf2b62a9c305a86ca3f1"
  end

  on_intel do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "5e0ef35b07da5f1cd590641c58f042f603b0f7314bc3ebe29689b0b945b369fc"
  end

  def install
    bin.install "listen"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/listen --version")
  end
end
