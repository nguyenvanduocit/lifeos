# lifeos

AI agent quản trị cuộc đời: việc và mục tiêu, thói quen, mối quan hệ, tài chính, sức khoẻ, nhật ký, ghi chú và inbox. Dữ liệu nằm trên máy người dùng; người dùng trò chuyện với agent qua CLI, web, Telegram và MCP.

## Kiến trúc

```
crates/
  lifeos-wire        kiểu dữ liệu đi qua đường truyền và frame postcard (sans-IO, biên dịch cả wasm32)
  lifeos-core        vault Markdown + SQLite + bộ tool (`tools!`): mọi thao tác đều đi qua đây
                     src/life/: mỗi mảng một module (tasks, goals, habits, people, money,
                     health, notes, inbox), crew, memory, chats và dashboard; input của tool
                     khai báo cạnh hàm của nó
  lifeos-agent       phiên Claude Code (crate claude-code-agent) với tool lifeos chạy trong process
  lifeos-mcp         bộ tool lifeos làm MCP server qua stdio
  lifeos-web         axum (app nhúng + /link.json) và WebTransport (web-transport-quinn) trên UDP
  lifeos-web-client  phía trang của link, biên dịch sang wasm (web-transport, wasm-bindgen)
  lifeos-telegram    bot Telegram dùng long polling
  lifeos             binary: init | today | reindex | chat | ask | mcp | web | telegram
                     (`today` in dashboard dạng JSON, không cần AI)
web/                 Vue 3 + Vite + Tailwind 4, chỉ vẽ những gì client wasm đưa cho
```

Đường đi của trang web: Vue ⇄ wasm (`lifeos-web-client`) ⇄ WebTransport, frame = 4 byte độ dài big-endian + postcard ⇄ `lifeos-web` ⇄ `Life` / `LifeAgent`. Mỗi lệnh gọi dùng một bidi stream riêng: client ghi một `Call` rồi finish; server ghi các `Reply::Progress(Step)` (chữ, suy nghĩ, tool gọi, kết quả tool), sau đó đúng một reply cuối, rồi finish. Riêng `Call::Watch` giữ stream mở và gửi `Reply::Changed` mỗi khi dữ liệu đổi (agent ghi, chat xong, hoặc tiến trình khác sửa vault hay database).

## Lấy agent làm trung tâm (như remit)

- Trang web chỉ **xem và nói**. Nó đọc qua `Call::Read` với tool `read`; server từ chối mọi tool `write` từ trang. Mọi thay đổi đi qua agent.
- Không có form hay trình soạn thảo. Nút trên trang hoặc gửi một câu cho agent (`ask`), hoặc đặt câu vào ô Assistant để người dùng viết nốt (`say`, qua `AskChip`).
- Trò chuyện nằm trong bảng Assistant cạnh mọi trang, không gắn chat vào chân từng trang. Bảng nói được với Assistant hoặc một thành viên crew (`Call::Chat { agent }`, `Stop`, `History`, `ResetChat`); mỗi agent một phiên Claude Code và một lịch sử riêng (bảng `turns`). Cách vẽ theo remit: tin của người dùng là bong bóng, các bước của một lượt xong thì gấp lại sau "Đã làm trong…", tool liền nhau gộp một hàng, lỗi luôn hiện.
- **Crew**: thành viên là file trong `crew/` (vai trò, hướng dẫn ngôi thứ hai, model, effort, tập tool). Chỉ Assistant tạo, đổi hay cho nghỉ thành viên (`add_member`, `update_member`, `retire_member`); trang chỉ xem. Đổi cấu hình thì phiên của thành viên được dựng lại.
- **Memory**: bảng `memories` + `memories_fts`, mỗi memory có `holder` (`assistant` hoặc id thành viên). Agent tự ghi bằng `remember`/`forget`; trước mỗi tin nhắn lifeos tự recall tối đa 5 memory liên quan (của chính nó và của Assistant) và đặt vào khối `<recalled>`. Trang Memory chỉ xem; "Quên…" là một câu gửi agent.
- Những gì agent làm phải hiện ngay: tool đang chạy hiện thành chip trong Assistant, dữ liệu đổi thì trang tự đọc lại (`useRead` theo `revision`).
- Bản tin buổi sáng do agent viết và lưu bằng `save_review` (kind `brief`); `lifeos web --brief-at 06:30` hẹn giờ viết mỗi ngày.

## Quy tắc

- **Mỗi kiểu dữ liệu chỉ định nghĩa một lần.** Kiểu đi qua đường truyền và kiểu view nằm trong `lifeos-wire`; input của tool khai báo bằng macro `input!` trong `lifeos-core`. TypeScript được sinh ra bằng `make gen` (ts-rs → `web/src/core/generated/`, kèm `tools.ts` ánh xạ mỗi tool `read` sang input và output của nó). Không chép kiểu bằng tay sang TS và không sửa file đã sinh.
- **Postcard không tự mô tả.** Kiểu trong `lifeos-wire` không được dùng `skip_serializing_if`, `flatten`, enum untagged hay internally tagged, hay `serde_json::Value`. Mỗi kiểu mới phải có trong `crates/lifeos-wire/tests/round_trip.rs`.
- **Mỗi tool chỉ khai báo một lần**, trong `tools!` (`crates/lifeos-core/src/tools.rs`): quyền `read`/`write`, tên, kiểu input và output, mô tả, handler. Agent, MCP và web đều đọc `specs()` và gọi `call()`; `call()` tự `sync_index()` trước.
- Lựa chọn cố định (trạng thái, độ ưu tiên…) khai báo bằng `choice!` để schema, parser và TS cùng một nguồn.
- **Mỗi loại dữ liệu có một nguồn gốc duy nhất.** Markdown trong vault giữ phần văn bản (tasks, goals, journal, notes); bảng `docs`/`docs_fts` chỉ là index, xoá đi dựng lại được (`lifeos reindex`). SQLite giữ phần số liệu (`habits`, `habit_logs`, `interactions`, `transactions`, `budgets`, `metrics`, `inbox`, `memories`, `turns`, `kv`). Tiền lưu theo đơn vị nhỏ nhất của từng loại tiền (`amount_minor`).
- Người dùng có thể sửa file trong vault bằng tay. Gọi `sync_index()` trước khi đọc index (`tools::call` đã làm sẵn).
- Agent không có Bash. Tool built-in bị giới hạn trong vault (`cwd`), và `PermissionMode::DontAsk` từ chối mọi thứ chưa được cho phép.
- Web và link không có đăng nhập, nên chỉ chạy trên loopback hoặc tailnet (`scripts/install-tailscale.sh`), không bao giờ mở ra internet. Telegram chỉ phục vụ các chat id được cho phép.
- Cert của link tự ký và được ghim bằng hash. Trình duyệt chỉ nhận cert ghim hash có hạn dưới 14 ngày, nên `cert::rotate` thay cert mới trên endpoint QUIC mỗi 6 ngày; session đang mở giữ cert cũ.
- Dự án còn sơ khai: refactor thoải mái, không cần giữ tương thích ngược.

## Lệnh

```sh
make deps        # npm install
make gen         # sinh lại TypeScript từ lifeos-wire và input của tool
make core        # build wasm vào web/src/core/pkg (yêu cầu wasm-bindgen-cli đúng bản đã ghim)
make test        # test Rust, build wasm, typecheck Vue
make dev         # backend :7420 (link udp :7421) + Vite :5173
make build       # binary release có nhúng web app
```

Kiểm tra đầu-cuối mà không cần AI: `cargo test -p lifeos-web --test link` dựng server thật và gọi qua QUIC thật.
