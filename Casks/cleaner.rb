cask "cleaner" do
  version "0.2.1"
  sha256 "3ca7c5778c16b8791e8019ca529fd26d8197569943dca6f73e03e2dfcde1529c"

  url "https://github.com/dreygur/cleaner/releases/download/v#{version}/Cleaner-#{version}-macos.zip"
  name "Cleaner"
  desc "Frees disk space by deleting build output and caches that tools can rebuild"
  homepage "https://github.com/dreygur/cleaner"

  depends_on arch: :arm64
  depends_on macos: :big_sur

  app "Cleaner.app"
  binary "#{appdir}/Cleaner.app/Contents/MacOS/cleaner"

  # The app is signed ad hoc, not with a Developer ID, so Gatekeeper would
  # refuse to open a downloaded copy. Clearing the quarantine flag lets it run.
  postflight_steps do
    run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "{{appdir}}/Cleaner.app"]
  end

  zap trash: "~/.config/cleaner"
end
