import AppKit
import Foundation

// Szöveg begépelése egy futó alkalmazás CGEvent-tel (Accessibility-engedéllyel).
// A virtuális billentyűkód NEM az Unicode érték — külön leképezés kell.
// Használat: swift scripts/ax-type.swift <app-nev> <szoveg>

let keyCodes: [Character: UInt16] = [
    "a": 0, "b": 11, "c": 8, "d": 2, "e": 14, "f": 3, "g": 5, "h": 4,
    "i": 34, "j": 38, "k": 40, "l": 37, "m": 46, "n": 45, "o": 31, "p": 35,
    "q": 12, "r": 15, "s": 1, "t": 17, "u": 32, "v": 9, "w": 13, "x": 7,
    "y": 16, "z": 6,
    "0": 29, "1": 18, "2": 19, "3": 20, "4": 21, "5": 23, "6": 22, "7": 26,
    "8": 28, "9": 25, " ": 49,
]

let args = Array(CommandLine.arguments.dropFirst())
guard args.count == 2 else {
    FileHandle.standardError.write(Data("használat: ax-type.swift <app-nev> <szoveg>\n".utf8))
    exit(2)
}
let running = NSWorkspace.shared.runningApplications.filter {
    ($0.localizedName ?? "").localizedCaseInsensitiveContains(args[0])
        && $0.activationPolicy == .regular
}
guard let app = running.first else {
    FileHandle.standardError.write(Data("nem fut ilyen app: \(args[0])\n".utf8))
    exit(1)
}
app.activate()
usleep(300_000)

let source = CGEventSource(stateID: .combinedSessionState)
var typed = 0
for character in args[1].lowercased() {
    guard let code = keyCodes[character] else { continue }
    guard let down = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true),
          let up = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: false)
    else { continue }
    down.flags = []
    up.flags = []
    down.postToPid(app.processIdentifier)
    up.postToPid(app.processIdentifier)
    typed += 1
    usleep(30_000)
}
print("begepelve: \(args[1]) (\(typed) billentyű)")
