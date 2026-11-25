import SwiftUI
import WebKit

/// The LifeOS web app, as the browser shows it.
struct LifeView: View {
    var body: some View {
        WebView(url: Settings.server).ignoresSafeArea(edges: .bottom)
    }
}

private struct WebView: UIViewRepresentable {
    let url: URL

    func makeUIView(context: Context) -> WKWebView {
        let view = WKWebView(frame: .zero, configuration: WKWebViewConfiguration())
        view.allowsBackForwardNavigationGestures = true
        view.scrollView.refreshControl = UIRefreshControl()
        view.scrollView.refreshControl?.addAction(UIAction { [weak view] _ in
            view?.reload()
            view?.scrollView.refreshControl?.endRefreshing()
        }, for: .valueChanged)
        view.load(URLRequest(url: url))
        return view
    }

    func updateUIView(_ view: WKWebView, context: Context) {}
}
