# LifeOS plugins

LifeOS uses Remit's Rust SDK, stdio protocol (contract 3) and child-process runner. Build a plugin with `plugin-sdk` and call `plugin_sdk::serve` from its entry point. A plugin runs as the LifeOS service user with a cleared environment. Install binaries you trust in an owner-controlled directory and pin their SHA-256. LifeOS checks the pin at every process start. A disconnected call fails and the next call can start a new process.

GitHub is available as an [optional plugin](github-plugin.md).

## Install a plugin

Build the example with `cargo build --release -p plugin-sdk --example hello`. Copy the executable to the path you intend to keep and compute its hash with `shasum -a 256 /absolute/path/to/hello`.

Add an entry to `$LIFEOS_HOME/.lifeos/plugins.json` (default home `~/lifeos`). Preserve other entries when editing the file.

```json
{
  "hello": {
    "binary": "/absolute/path/to/hello",
    "sha256": "<64 hex digits>",
    "functions": ["latest_event"],
    "hooks": { "echo": "echo" },
    "events": {
      "lifeos.started": "remember",
      "lifeos.changed": "remember",
      "plugin.hello.message": "remember"
    }
  }
}
```

Restart the LifeOS server to apply changes. Remove the entry and restart to uninstall. Optional `args` are passed to the executable. Optional `vars` are strings passed to tool and processor calls. Restrict the file to the service user (for example, mode `0600`) if it contains credentials. Functions, hooks and event handlers are enabled by the owner's config. Keep the server on the private network used for LifeOS.

## Backend

- `GET /api/plugins` lists installed plugins and their enabled functions, frontend slots and startup errors. Each entry includes `retrying_deliveries`, the number of outstanding subscriber deliveries that have failed at least once.
- `POST /api/plugins/{id}` accepts the JSON string `"install"`, `"enable"` or `"disable"` and the header `X-LifeOS-Plugin: 1`. Install is available for staged packages in the catalog.
- `POST /api/plugins/{id}/functions/{tool}` calls a manifest tool with a body such as `{"arguments":{}}`.
- `POST /api/plugins/{id}/hooks/{name}` invokes the configured processor with a JSON object as its parameters. For the example, send `{"message":"Hello"}` to `/api/plugins/hello/hooks/echo`.
- A hook can instead use an object binding with `processor` and `headers`. The processor receives `{"body":"…","headers":{…}}` with the original UTF-8 body and only the named lowercase headers. This allows a plugin to verify a signed request before processing it. The body limit is 2 MiB.
- Event handlers receive the event in `ProcessInput.reading` and an empty object in `params`. `lifeos.started` fires when the server starts. `lifeos.changed` carries the current web revision and can coalesce rapid changes. A processor can emit readings under `plugin.<id>.<keySuffix>`; a configured handler can subscribe to these keys. LifeOS commits emitted events and subscriber deliveries to `.lifeos/plugin-events.sqlite` before acknowledging the hook. HTTP success means the hook processor finished and its events were accepted. Subscribers run asynchronously. A nonempty `Reading.dedupe` suppresses repeat events with the same full key and token for 30 days. Each subscriber receives a stable `request_id` across retries and service restarts. Use this ID for side effects in external systems. Failed deliveries retry with exponential backoff up to 256 seconds between attempts. The worker skips bindings removed or disabled before delivery. Each chain allows up to 64 events and eight levels.

## Frontend

Declare a `page` slot for Integrations → Plugins or a `dashboard` slot for Today. Set `expose` to an HTML asset path, or leave it empty to use `Ui.entry`. Each slot runs in a sandboxed iframe. The asset endpoint uses the MIME type supplied by the plugin, falling back to the filename extension for an empty type. Invalid MIME declarations return HTTP 502. From the iframe, send `{type: "lifeos:call", id, name, arguments}` to `parent.postMessage`. The host answers with `{type: "lifeos:answer", id, result}` or `error`. The host also sends `{type: "lifeos:event", name, revision}` for `lifeos.ready` and `lifeos.changed`. The [example](../crates/plugin-sdk/examples/hello.rs) demonstrates these entry points.

LifeOS implements the host bindings above. Remit's Module Federation UI, agent grants, credential proxy, account and forge services, and automatic poll and sensor scheduling require separate host integrations.

A slot that should fit its content can send `{type: "lifeos:resize", height}` with the measured content height in CSS pixels. The host caps the requested content height at 1600 pixels. A `ResizeObserver` on the content can report changes when text wraps.
