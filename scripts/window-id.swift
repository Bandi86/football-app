import CoreGraphics
import Foundation

// A megadott tulajdonosnevű (részleges egyezés) legfelső ablaka azonosítóját írja ki.
// A `screencapture -l<id>` ezzel tudja kimenteni a futó app ablakát (Screen Recording kell hozzá).
let owner = CommandLine.arguments.dropFirst().first ?? "FootballApp"
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows {
    guard let name = window[kCGWindowOwnerName as String] as? String,
          name.localizedCaseInsensitiveContains(owner),
          let number = window[kCGWindowNumber as String] as? Int,
          (window[kCGWindowLayer as String] as? Int ?? 1) == 0
    else { continue }
    print(number)
    exit(0)
}
FileHandle.standardError.write(Data("nincs ilyen ablak: \(owner)\n".utf8))
exit(1)
