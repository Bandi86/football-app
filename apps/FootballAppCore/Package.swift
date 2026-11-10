// swift-tools-version: 6.0
import Foundation
import PackageDescription

// A Command Line Tools-ban a Swift Testing makró plugin a `host/plugins/testing`
// alkönyvtárban van, amit a driver alapból nem keres, ezért explicit
// `-plugin-path` kapcsolóval adjuk át a tesztcél fordításához.
let developerDir = ProcessInfo.processInfo.environment["DEVELOPER_DIR"]

let swiftTestingPluginCandidates = [
    developerDir.map { "\($0)/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/host/plugins/testing" },
    "/Library/Developer/CommandLineTools/usr/lib/swift/host/plugins/testing",
    "/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/host/plugins/testing",
    "/Applications/Xcode-beta.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/host/plugins/testing",
].compactMap { $0 }

var testSwiftSettings: [SwiftSetting] = []
if let pluginPath = swiftTestingPluginCandidates.first(where: {
    FileManager.default.fileExists(atPath: $0 + "/libTestingMacros.dylib")
}) {
    testSwiftSettings.append(.unsafeFlags(["-plugin-path", pluginPath]))
}

let package = Package(
    name: "FootballAppCore",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "FootballAppCore", targets: ["FootballAppCore"])
    ],
    targets: [
        .target(name: "FootballAppCore"),
        .testTarget(
            name: "FootballAppCoreTests",
            dependencies: ["FootballAppCore"],
            swiftSettings: testSwiftSettings
        )
    ]
)
