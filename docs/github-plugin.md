# GitHub plugin

GitHub is optional. The plugin packages the GitHub MCP connection settings and translates signed GitHub webhook deliveries into LifeOS plugin events. Existing GitHub MCP connections keep their saved settings and crew access. LifeOS shows them in Connections and supplies them to agents while the GitHub plugin is enabled.

## Install

If the LifeOS host has staged the GitHub package, open **Integrations → Plugins** and click **Install**. Use **Disable** and **Enable** to stop and resume the plugin without restarting LifeOS. For webhook configuration or manual installs, use the host configuration below.

1. Build with `cargo build --release -p lifeos-github`, copy the resulting executable to an owner-controlled directory on the LifeOS host, and compute its SHA-256 with `shasum -a 256 /path/to/lifeos-github`.
2. Merge this entry into `$LIFEOS_HOME/.lifeos/plugins.json` and restart LifeOS. The plugin appears under **Integrations → Plugins**.

   ```json
   {
     "github": {
       "binary": "/path/to/lifeos-github",
       "sha256": "<64 hex digits>",
       "hooks": {
         "webhook": {
           "processor": "webhook",
           "headers": ["x-github-event", "x-github-delivery", "x-hub-signature-256"]
         }
       },
       "vault_keys": ["GITHUB_WEBHOOK_SECRET"]
     }
   }
   ```

   Store GITHUB_WEBHOOK_SECRET in Vault and list its name under `vault_keys`. For MCP-only use, omit the webhook binding.

## Give agents access

With GitHub enabled, select the GitHub preset in Connections and save GITHUB_TOKEN in Vault. Existing connections retain their names and crew access. The host CLI also supports provisioning the connection.

```sh
/path/to/lifeos-github --mcp-config | lifeos connection set github
```

Add `GITHUB_TOKEN` to the Vault and grant access to the agents that need it. The connection uses the existing LifeOS Vault and crew permissions. Disabling the plugin pauses its hooks and MCP access and hides its Connections entry. Enabling it restores the saved connection and permissions. Remove the separately provisioned MCP connection with `lifeos connection remove github` if you also want to revoke that connection.

## Receive GitHub events

Configure a GitHub webhook with content type `application/json` and the same shared secret. Its destination must forward `POST /api/plugins/github/hooks/webhook` and selected headers to LifeOS. GitHub.com needs a publicly reachable relay when LifeOS is tailnet-only. Forward only the webhook path and preserve the body bytes. The LifeOS hook accepts UTF-8 bodies up to 2 MiB and checks the HMAC-SHA256 signature before parsing the payload. [GitHub webhook verification](https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries)

Each delivery emits `plugin.github.<event>` with `delivery_id`, `event` and `payload`. For example, add `"plugin.github.pull_request": "handle_pull_request"` to another installed plugin's `events` map. `push`, `issues` and `workflow_run` follow the same pattern. LifeOS suppresses repeat deliveries for the same event and delivery ID for 30 days. Subscriber retries carry a stable request ID that handlers can use to make external side effects idempotent.

Subscribe to `plugin.github.pull_request_merged` for completed merges. Enable the **Pull requests** webhook event for this signal. **Pull request reviews** and **Pull request review comments** feed `plugin.github.pull_request_review` and `plugin.github.pull_request_review_comment`.

For personal inbox updates on macOS, configure the signed webhook above, authenticate GitHub CLI on the host with access to notifications, then run `python3 scripts/install-github-notifications.py`. The collector forwards the latest unread thread at setup and checks for thread updates every five minutes. Subscribe to `plugin.github.notification`. Its payload contains the thread ID, repository, subject, reason and update time. The collector keeps its checkpoint in the LifeOS home and preserves GitHub read status.
