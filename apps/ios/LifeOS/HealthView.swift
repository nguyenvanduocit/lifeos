import SwiftUI

/// What the phone sends to LifeOS and when it last did.
struct HealthView: View {
    @Environment(HealthSync.self) private var sync
    @State private var server = Settings.server.absoluteString
    @State private var confirmHistory = false

    var body: some View {
        NavigationStack {
            Form {
                if !sync.available {
                    Section { Text("Thiết bị này không có Apple Health.").foregroundStyle(.secondary) }
                }
                Section("Đồng bộ") {
                    LabeledContent("Lần cuối") {
                        if let last = sync.lastSync {
                            Text(last, format: .relative(presentation: .named))
                        } else {
                            Text("Chưa").foregroundStyle(.secondary)
                        }
                    }
                    if let error = sync.error {
                        Text(error).font(.footnote).foregroundStyle(.red)
                    }
                    Button {
                        Task { await sync.run(reason: .asked) }
                    } label: {
                        HStack {
                            Text("Đồng bộ ngay")
                            if sync.running { Spacer(); ProgressView() }
                        }
                    }
                    .disabled(sync.running || !sync.available)
                    Button("Gửi lại cả năm qua") { confirmHistory = true }
                        .disabled(sync.running || !sync.available)
                }
                if !sync.today.isEmpty {
                    Section("Hôm nay") {
                        ForEach(sync.today, id: \.self) { reading in
                            LabeledContent(reading.measure.label) {
                                Text("\(reading.value.formatted(.number.precision(.fractionLength(0...2)))) \(reading.measure.unit)")
                            }
                        }
                    }
                }
                Section {
                    TextField("https://lifeos.aiocean.dev", text: $server)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onSubmit(saveServer)
                } header: {
                    Text("Máy chủ LifeOS")
                } footer: {
                    Text("Điện thoại cần ở trong tailnet. LifeOS giữ một giá trị mỗi ngày cho mỗi chỉ số; gửi lại một ngày sẽ thay giá trị cũ.")
                }
            }
            .navigationTitle("Sức khoẻ")
            .confirmationDialog("Gửi lại dữ liệu 365 ngày qua?", isPresented: $confirmHistory, titleVisibility: .visible) {
                Button("Gửi lại") { Task { await sync.run(reason: .history) } }
            }
        }
    }

    private func saveServer() {
        let trimmed = server.trimmingCharacters(in: .whitespaces)
        guard let url = URL(string: trimmed), url.scheme == "https", url.host() != nil else {
            server = Settings.server.absoluteString
            return
        }
        Settings.server = url
    }
}
