import AppKit
import ApplicationServices
import Foundation

// Az app AX-fájából kiírja azokat az elemeket, amelyek azonosítója/leírása
// illeszkedik a szűrőre, szerepükkel és értékükkel együtt.
// Használat: ax-probe.swift <app-nev> [szuro]

func value(_ element: AXUIElement, _ name: String) -> String? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &result) == .success else { return nil }
    return result as? String
}

func children(of element: AXUIElement) -> [AXUIElement] {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &result) == .success,
          let array = result as? [AXUIElement]
    else { return [] }
    return array
}

let arguments = Array(CommandLine.arguments.dropFirst())
guard let appName = arguments.first else {
    FileHandle.standardError.write(Data("használat: ax-probe.swift <app-nev> [szuro]\n".utf8))
    exit(2)
}
let needle = arguments.count > 1 ? arguments[1] : "ui.focusedMatch"

let running = NSWorkspace.shared.runningApplications.filter {
    ($0.localizedName ?? "").localizedCaseInsensitiveContains(appName) && $0.activationPolicy == .regular
}
guard let app = running.first else {
    FileHandle.standardError.write(Data("nem fut ilyen app: \(appName)\n".utf8))
    exit(1)
}

let appElement = AXUIElementCreateApplication(app.processIdentifier)
var stack: [(AXUIElement, Int)] = [(appElement, 0)]
while let (element, depth) = stack.popLast() {
    let identifier = value(element, kAXIdentifierAttribute) ?? ""
    let description = value(element, kAXDescriptionAttribute) ?? ""
    let title = value(element, kAXTitleAttribute) ?? ""
    if identifier.localizedCaseInsensitiveContains(needle)
        || description.localizedCaseInsensitiveContains(needle) {
        let role = value(element, kAXRoleAttribute) ?? "?"
        let axValue = value(element, kAXValueAttribute) ?? ""
        print("\(role) id=\(identifier) desc=\(description) title=\(title) value=\(axValue)")
    }
    for child in children(of: element).reversed() {
        stack.append((child, depth + 1))
    }
}