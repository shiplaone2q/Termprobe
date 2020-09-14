class TuiTest < Formula
  desc "Control, inspect, test, and record terminal sessions"
  homepage "https://github.com/microsoft/tui-test"
  version "0.1.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/microsoft/tui-test/releases/download/0.1.1/tui-test-aarch64-apple-darwin.tar.gz"
      sha256 "a6819562a18031b212565f771d03fe9e5ec7d4f71b389394fd909cc66cee6b81"
    end

    on_intel do
      url "https://github.com/microsoft/tui-test/releases/download/0.1.1/tui-test-x86_64-apple-darwin.tar.gz"
      sha256 "298ae3d37549d4712a7795d388f4e871f3bf8b7863372c3441787870c747408d"
