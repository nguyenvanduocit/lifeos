import Foundation

/// What the app keeps between launches, in UserDefaults.
enum Settings {
    static let defaults = UserDefaults.standard

    /// The LifeOS server, reached over the tailnet.
    static var server: URL {
        get { defaults.url(forKey: "server") ?? URL(string: "https://lifeos.aiocean.dev")! }
        set { defaults.set(newValue, forKey: "server") }
    }

    /// The last time the server kept what the phone sent.
    static var lastSync: Date? {
        get { defaults.object(forKey: "lastSync") as? Date }
        set { defaults.set(newValue, forKey: "lastSync") }
    }
}
