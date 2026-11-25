import SwiftUI

@main
struct LifeOSApp: App {
    @State private var sync = HealthSync.shared
    @Environment(\.scenePhase) private var phase

    init() {
        // Observers must be in place before the app finishes launching, so HealthKit can wake it in the background.
        HealthSync.shared.observe()
    }

    var body: some Scene {
        WindowGroup {
            TabView {
                LifeView()
                    .tabItem { Label("LifeOS", systemImage: "sparkles") }
                HealthView()
                    .tabItem { Label("Sức khoẻ", systemImage: "heart.text.square") }
            }
            .environment(sync)
        }
        .onChange(of: phase) { _, now in
            if now == .active { Task { await sync.run(reason: .opened) } }
        }
    }
}
