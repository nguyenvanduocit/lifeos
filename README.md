# lifeos

AI agent giúp bạn quản trị cuộc đời: việc cần làm, mục tiêu, thói quen, mối quan hệ, tiền bạc, sức khoẻ, nhật ký, ghi chú và inbox. Dữ liệu là file Markdown và một file SQLite trên máy bạn. Agent là một phiên Claude Code chạy trên thư mục dữ liệu đó.

Web là một dashboard để nhìn toàn cảnh: bản tin buổi sáng, việc hôm nay, chuỗi thói quen, ai cần liên lạc, chi tiêu so với ngân sách, xu hướng sức khoẻ. Dashboard chỉ để xem; muốn thay đổi gì thì nói với agent trong bảng Assistant bên cạnh, và trang tự cập nhật khi agent làm xong.

## Cần có

- Rust (nightly, edition 2024), target `wasm32-unknown-unknown`, và `wasm-bindgen-cli` đúng bản crate đang ghim (`cargo install wasm-bindgen-cli --version 0.2.130 --locked`)
- Node.js và npm
- Claude Code 2.1.329 trở lên, đã đăng nhập (`claude update`)

## Bắt đầu

```sh
make deps build
./target/release/lifeos init          # tạo ~/lifeos (đổi bằng --home hoặc LIFEOS_HOME)
$EDITOR ~/lifeos/CLAUDE.md            # viết về bạn; agent đọc file này mỗi lần trò chuyện
```

## Dùng

| Kênh | Lệnh |
|---|---|
| Terminal | `lifeos chat` (thêm `-r` để tiếp tục cuộc trước), `lifeos ask "…"`, `lifeos today` |
| Web | `lifeos web`, rồi mở http://127.0.0.1:7420 (thêm `--brief-at 06:30` để agent viết bản tin mỗi sáng) |
| Telegram | `LIFEOS_TELEGRAM_TOKEN=… LIFEOS_TELEGRAM_ALLOWED_CHATS=<chat id> lifeos telegram` |
| MCP | `claude mcp add lifeos -- lifeos mcp` |

### Mở web qua Tailscale

```sh
make build && scripts/install-tailscale.sh
```

Lệnh này cài `lifeos web` thành LaunchAgent (bản tin buổi sáng lúc 06:30, đổi bằng `BRIEF_AT=07:00`, tắt bằng `BRIEF_AT=`), tự khởi động cùng máy và tự chạy lại khi chết. App mở ở `https://<máy>.<tailnet>.ts.net:8443` qua `tailscale serve`, chỉ trong tailnet. Link WebTransport (UDP 7421) nghe thẳng trên IP Tailscale, vì `tailscale serve` không chuyển UDP. Cert của link tự xoay mỗi 6 ngày. Log ở `~/Library/Logs/lifeos-web.log`. Web không có đăng nhập: ai trong tailnet của bạn cũng mở được.

Gỡ: `launchctl bootout gui/$(id -u)/com.lifeos.web && tailscale serve --https=8443 off`.

Chưa biết chat id Telegram của mình? Cứ chạy bot rồi nhắn cho nó; bot sẽ trả lời chat id của bạn.

Chọn model và mức suy nghĩ bằng `--model` / `LIFEOS_MODEL` và `--effort` / `LIFEOS_EFFORT`.

## Vault

```
~/lifeos/
  CLAUDE.md                 về bạn
  tasks/*.md  goals/*.md  people/*.md  notes/*.md
  reviews/2026-11-25-brief.md   bản tin và review agent viết
  journal/2026/2026-11-25.md
  .lifeos/lifeos.db         thói quen, liên lạc, giao dịch, ngân sách, chỉ số, inbox, index tìm kiếm — hãy sao lưu
```

Bạn có thể sửa file Markdown bằng bất kỳ trình soạn thảo nào, kể cả Obsidian; lifeos tự cập nhật lại index.

Kiến trúc và quy tắc phát triển: xem `CLAUDE.md`.
