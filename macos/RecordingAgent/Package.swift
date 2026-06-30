// swift-tools-version: 5.10

import PackageDescription

let package = Package(
    name: "RecordingAgent",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .library(name: "RecordingAgentCore", targets: ["RecordingAgentCore"]),
        .executable(name: "RecordingAgent", targets: ["RecordingAgent"]),
        .executable(name: "RecordingAgentCoreSelfTests", targets: ["RecordingAgentCoreSelfTests"]),
        .executable(name: "RecordingAgentSmoke", targets: ["RecordingAgentSmoke"])
    ],
    dependencies: [
        .package(url: "https://github.com/argmaxinc/argmax-oss-swift.git", exact: "1.0.0")
    ],
    targets: [
        .target(
            name: "RecordingAgentCore",
            linkerSettings: [
                .linkedLibrary("sqlite3")
            ]
        ),
        .executableTarget(
            name: "RecordingAgent",
            dependencies: [
                "RecordingAgentCore",
                .product(name: "WhisperKit", package: "argmax-oss-swift")
            ]
        ),
        .executableTarget(
            name: "RecordingAgentCoreSelfTests",
            dependencies: ["RecordingAgentCore"]
        ),
        .executableTarget(
            name: "RecordingAgentSmoke",
            dependencies: [
                "RecordingAgentCore",
                .product(name: "WhisperKit", package: "argmax-oss-swift")
            ]
        ),
        .testTarget(
            name: "RecordingAgentCoreTests",
            dependencies: ["RecordingAgentCore"]
        )
    ]
)
