import AppKit
import ApplicationServices
import Foundation

// Billentyű-eseményt küld egy futó alkalmazásnak, majd kiírja a fókuszált AX-elemet.
// Accessibility-engedély kell hozzá.
// Használat: ax-key.swift <app-nev> <tab|shift-tab|down|up|left|right|return|escape> [ismetles]

let keys: [String: (UInt16, CGEventFlags)] = [
    "tab": (48, []),
    "shift-tab": (48, .maskShift),
    "down": (125, []),
    "up": (126, []),
    "left": (123, []),
    "right": (124, []),
    "return": (36, []),
    "escape": (53, []),
    "cmd-left": (123, .maskCommand),
    "cmd-right": (124, .maskCommand),
    "cmd-t": (17, .maskCommand),
    "cmd-r": (15, .maskCommand),
    "cmd-f": (3, .maskCommand),
]

func attribute(_ element: AXUIElement, _ name: String) -> String? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value as? String
}

let arguments = Array(CommandLine.arguments.dropFirst())
guard arguments.count >= 2, let (virtualKey, flags) = keys[arguments[1].lowercased()] else {
    FileHandle.standardError.write(Data("használat: ax-key.swift <app-nev> <billentyű> [ismetles]\n".utf8))
    exit(2)
}
let appName = arguments[0]
let repeats = max(1, Int(arguments.count > 2 ? arguments[2] : "1") ?? 1)

let running = NSWorkspace.shared.runningApplications.filter {
    ($0.localizedName ?? "").localizedCaseInsensitiveContains(appName) && $0.activationPolicy == .regular
}
guard let app = running.first else {
    FileHandle.standardError.write(Data("nem fut ilyen app: \(appName)\n".utf8))
    exit(1)
}
app.activate()
usleep(300_000)

let source = CGEventSource(stateID: .combinedSessionState)
for _ in 0..<repeats {
    let down = CGEvent(keyboardEventSource: source, virtualKey: virtualKey, keyDown: true)
    down?.flags = flags
    let up = CGEvent(keyboardEventSource: source, virtualKey: virtualKey, keyDown: false)
    up?.flags = flags
    down?.postToPid(app.processIdentifier)
    up?.postToPid(app.processIdentifier)
    usleep(180_000)
}
usleep(250_000)

let system = AXUIElementCreateSystemWide()
var focused: CFTypeRef?
guard AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
      let element = focused as! AXUIElement?
else {
    print("fókuszált elem: nincs (AX nem adja)")
    exit(1)
}

let role = attribute(element, kAXRoleAttribute) ?? "?"
let description = attribute(element, kAXDescriptionAttribute) ?? ""
let title = attribute(element, kAXTitleAttribute) ?? ""
print("fókuszált: \(role) | \(description) | \(title)")