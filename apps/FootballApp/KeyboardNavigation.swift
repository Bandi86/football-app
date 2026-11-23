import AppKit
import SwiftUI

/// Billentyűzet-kódok, hogy a hívó ne mágikus számokat használjon.
enum KeyCode {
    static let tab: UInt16 = 48
    static let returnKey: UInt16 = 36
    static let enter: UInt16 = 76
    static let escape: UInt16 = 53
    static let up: UInt16 = 126
    static let down: UInt16 = 125
    static let left: UInt16 = 123
    static let right: UInt16 = 124
    /// ⌘T — ugrás a mai napra.
    static let today: UInt16 = 17
    /// ⌘R — a nap újratöltése.
    static let refresh: UInt16 = 15
    /// ⌘F — globális keresés (kVK_ANSI_F).
    static let search: UInt16 = 3
}

/// Helyi billentyűfigyelő a SwiftUI nézet mögött.
///
/// Miért kell: macOS-en a Tab-fókusz a rendszer „teljes billentyűzet-vezérlés”
/// beállításától függ, és a SwiftUI fókuszlánca nem ad determinisztikus,
/// tesztelhető sorrendet a napi listában. A monitor csak a saját ablakunkban
/// (és csak nyitott sheet nélkül) figyel, a kezelt eseményt elfogyasztja,
/// minden mást változatlanul továbbenged.
struct KeyboardMonitor: NSViewRepresentable {
    /// A kezelő a fő szálon fut; a billentyűkódot és a módosítókat kapja.
    let handler: (UInt16, NSEvent.ModifierFlags) -> Bool

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        context.coordinator.install()
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.handler = handler
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.remove()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(handler: handler)
    }

    @MainActor
    final class Coordinator {
        var handler: (UInt16, NSEvent.ModifierFlags) -> Bool
        private var monitor: Any?

        init(handler: @escaping (UInt16, NSEvent.ModifierFlags) -> Bool) {
            self.handler = handler
        }

        func install() {
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                guard let self else { return event }
                let keyCode = event.keyCode
                let modifiers = event.modifierFlags
                let consumed = MainActor.assumeIsolated { self.process(keyCode: keyCode, modifiers: modifiers) }
                return consumed ? nil : event
            }
        }

        /// A helyi monitor a fő szálon hív, ezért az AppKit-állapot itt olvasható.
        private func process(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> Bool {
            // Nyitott sheet fölött (a sheet saját ablakában) nem nyúlunk a billentyűkhöz.
            guard let window = NSApp.keyWindow,
                  window.attachedSheet == nil,
                  window.sheetParent == nil
            else { return false }
            return handler(keyCode, modifiers)
        }

        func remove() {
            if let monitor {
                NSEvent.removeMonitor(monitor)
            }
            monitor = nil
        }
    }
}