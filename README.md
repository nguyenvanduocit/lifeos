# lifeos

AI agent giúp bạn quản trị cuộc đời: mục tiêu, việc cần làm, thói quen, sức khoẻ, tiền bạc, nhật ký và ghi chú. Dữ liệu là file Markdown và một file SQLite trên máy bạn. Agent là một phiên Claude Code chạy trên thư mục dữ liệu đó.

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
| Web | `lifeos web`, rồi mở http://127.0.0.1:7420 |
| Telegram | `LIFEOS_TELEGRAM_TOKEN=… LIFEOS_TELEGRAM_ALLOWED_CHATS=<chat id> lifeos telegram` |
| MCP | `claude mcp add lifeos -- lifeos mcp` |

Chưa biết chat id Telegram của mình? Cứ chạy bot rồi nhắn cho nó; bot sẽ trả lời chat id của bạn.

Chọn model và mức suy nghĩ bằng `--model` / `LIFEOS_MODEL` và `--effort` / `LIFEOS_EFFORT`.

## Vault

```
~/lifeos/
  CLAUDE.md                 về bạn
  goals/*.md  tasks/*.md  notes/*.md
  journal/2026/2026-11-25.md
  .lifeos/lifeos.db         thói quen, giao dịch, chỉ số sức khoẻ, index tìm kiếm — hãy sao lưu
```

Bạn có thể sửa file Markdown bằng bất kỳ trình soạn thảo nào, kể cả Obsidian; lifeos tự cập nhật lại index.

Kiến trúc và quy tắc phát triển: xem `CLAUDE.md`.
