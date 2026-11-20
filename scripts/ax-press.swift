import AppKit
import ApplicationServices
import Foundation

// Egy futó alkalmazás AX-fájában megkeres egy elemet a leírása (AXDescription)
// vagy címe (AXTitle) alapján, és AXPress-t küld rá. Accessibility-engedély kell hozzá.
// Használat: ax-press.swift <app-nev> <keresett-szoveg> [--list]

func attribute(_ element: AXUIElement, _ name: String) -> String? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value as? String
}

func children(of element: AXUIElement) -> [AXUIElement] {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &value) == .success,
          let array = value as? [AXUIElement]
    else { return [] }
    return array
}

func walk(_ element: AXUIElement, depth: Int = 0, visit: (AXUIElement, Int) -> Bool) -> Bool {
    if visit(element, depth) { return true }
    for child in children(of: element) {
        if walk(child, depth: depth + 1, visit: visit) { return true }
    }
    return false
}

let arguments = CommandLine.arguments.dropFirst()
guard let appName = arguments.first, let needle = arguments.dropFirst().first else {
    FileHandle.standardError.write(Data("használat: ax-press.swift <app-nev> <szoveg> [--list]\n".utf8))
    exit(2)
}
let listOnly = arguments.contains("--list")

let running = NSWorkspace.shared.runningApplications.filter {
    ($0.localizedName ?? "").localizedCaseInsensitiveContains(appName) && $0.activationPolicy == .regular
}
guard let app = running.first else {
    FileHandle.standardError.write(Data("nem fut ilyen app: \(appName)\n".utf8))
    exit(1)
}

let appElement = AXUIElementCreateApplication(app.processIdentifier)
var found: AXUIElement?
_ = walk(appElement) { element, depth in
    let description = attribute(element, kAXDescriptionAttribute) ?? ""
    let title = attribute(element, kAXTitleAttribute) ?? ""
    let role = attribute(element, kAXRoleAttribute) ?? "?"
    if listOnly, !description.isEmpty || !title.isEmpty {
        print(String(repeating: " ", count: depth) + "\(role) | \(description) | \(title)")
    }
    if !listOnly, description.localizedCaseInsensitiveContains(needle) || title.localizedCaseInsensitiveContains(needle) {
        found = element
        return true
    }
    return false
}

if listOnly { exit(0) }
guard let target = found else {
    FileHandle.standardError.write(Data("nem található AX-elem: \(needle)\n".utf8))
    exit(1)
}
let result = AXUIElementPerformAction(target, kAXPressAction as CFString)
print("AXPress result: \(result.rawValue)")
exit(result == .success ? 0 : 1)
