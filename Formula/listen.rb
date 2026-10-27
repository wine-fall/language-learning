# Updated by .github/workflows/release.yml on every tagged release.
class Listen < Formula
  desc "Terminal app for intensive English listening practice with spaced-repetition review"
  homepage "https://github.com/wine-fall/language-learning"
  version "0.1.2"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "9c60615966c7cf35c40a06adb0b801dc158ef8326cbe31c8e8299003a905627b"
  end

  on_intel do
    url "https://github.com/wine-fall/language-learning/releases/download/v#{version}/listen-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "bd42ea9d1fd9e60e0fcd8bb86f56632da24e97df9967e94aa5db879425414cc4"
  end

  def install
    bin.install "listen"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/listen --version")
  end
end
