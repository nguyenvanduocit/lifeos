# Triển khai dashboard

## Trên máy local

```sh
./target/release/lifeos web
```

Mở [dashboard local](http://127.0.0.1:7420). Trang và endpoint `/link.json` dùng HTTP TCP 7420. Client Wasm kết nối WebTransport qua UDP 7421. Web và link không có đăng nhập tích hợp. Giữ chúng trên loopback hoặc giới hạn truy cập trong tailnet tin cậy.

## Qua Tailscale trên macOS

Cài và đăng nhập Tailscale, bật HTTPS cho tailnet, rồi build binary bằng [script setup](../scripts/setup.sh). Chạy:

```sh
BRIEF_AT=06:30 ./scripts/install-tailscale.sh
```

Script cài LaunchAgent để chạy ứng dụng cùng phiên đăng nhập macOS. Mở URL HTTPS mà script in ra, mặc định cổng 8443. Đặt `BRIEF_AT=` để tắt lịch bản tin hoặc đặt `HTTPS_PORT` để chọn cổng HTTPS khác. Chạy lại script sau khi build để cập nhật binary và khởi động lại dịch vụ.

Tailscale Serve chuyển HTTPS vào HTTP loopback. Trình duyệt kết nối UDP 7421 trực tiếp qua IP Tailscale, nên thiết bị truy cập cần vào được cả hai đường. Xem log tại `~/Library/Logs/lifeos-web.log`.

Nếu trang mở được nhưng Assistant không kết nối, kiểm tra `/link.json`, kết nối của cả hai thiết bị vào tailnet và quyền truy cập UDP 7421. Nếu dùng reverse proxy hoặc tên miền riêng, giữ cùng giới hạn truy cập qua tailnet cho HTTP và UDP.

Gỡ dịch vụ và cấu hình Tailscale Serve mặc định:

```sh
launchctl bootout "gui/$(id -u)/com.lifeos.web"
tailscale serve --https=8443 off
```
