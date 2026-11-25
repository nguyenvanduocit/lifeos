# Deploy the dashboard

## Run locally

```sh
./target/release/lifeos web
```

Open the [local dashboard](http://127.0.0.1:7420). The page and `/link.json` use HTTP on TCP port 7420. The Wasm client connects over WebTransport on UDP port 7421. The web app and WebTransport link have no built-in authentication. Bind both to loopback or restrict access to a trusted tailnet.

## Use Tailscale on macOS

Install and sign in to Tailscale, enable HTTPS for your tailnet, and build the binary with the [setup script](../scripts/setup.sh). Then run:

```sh
BRIEF_AT=06:30 ./scripts/install-tailscale.sh
```

The script installs a LaunchAgent so the app starts when you sign in to macOS. Open the HTTPS URL printed by the script. The default HTTPS port is 8443. Set `BRIEF_AT=` to disable the daily brief, or set `HTTPS_PORT` to select another HTTPS port. After rebuilding LifeOS, rerun the script to update the installed binary and restart the service.

Tailscale Serve forwards HTTPS to the local HTTP server. The browser connects directly to the server’s Tailscale IP on UDP port 7421, so the client must be able to reach both endpoints. Logs are available at `~/Library/Logs/lifeos-web.log`.

If the dashboard opens but the Assistant cannot connect, check `/link.json`, confirm both devices are connected to the tailnet, and verify access to UDP port 7421. Apply the same tailnet access restrictions to HTTP and UDP when using a reverse proxy or a custom domain.

To remove the service and the default Tailscale Serve configuration, run:

```sh
launchctl bootout "gui/$(id -u)/com.lifeos.web"
tailscale serve --https=8443 off
```
