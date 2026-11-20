import CoreGraphics
import Foundation

// A megadott tulajdonosnevű (részleges egyezés) legfelső ablaka azonosítóját írja ki.
// A `screencapture -l<id>` ezzel tudja kimenteni a futó app ablakát (Screen Recording kell hozzá).
// Argumentum: tulajdonosnév (részleges) vagy --pid <szám>.
var owner = "FootballApp"
var pid: pid_t?
var arguments = Array(CommandLine.arguments.dropFirst())
if arguments.first == "--pid", arguments.count > 1, let parsed = pid_t(arguments[1]) {
    pid = parsed
    arguments.removeFirst(2)
}
if let first = arguments.first, !first.isEmpty {
    owner = first
}
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows {
    guard let name = window[kCGWindowOwnerName as String] as? String,
          pid.map({ (window[kCGWindowOwnerPID as String] as? pid_t) == $0 })
              ?? name.localizedCaseInsensitiveContains(owner),
          let number = window[kCGWindowNumber as String] as? Int,
          (window[kCGWindowLayer as String] as? Int ?? 1) == 0
    else { continue }
    print(number)
    exit(0)
}
FileHandle.standardError.write(Data("nincs ilyen ablak: \(owner)\n".utf8))
exit(1)
