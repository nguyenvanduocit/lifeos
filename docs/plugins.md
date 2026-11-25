# LifeOS plugins

LifeOS uses Remit's Rust SDK, stdio protocol (contract 4) and child-process runner. Build a plugin with `plugin-sdk` and call `plugin_sdk::serve` from its entry point. A plugin runs as the LifeOS service user with a cleared environment. Install binaries you trust in an owner-controlled directory and pin their SHA-256. LifeOS checks the pin at every process start. A disconnected call fails and the next call can start a new process.

Contract 4 carries the manifest as a JSON document. Rebuild older external binaries with the current SDK and update their SHA-256 pins when upgrading from contract 3.

GitHub is available as an [optional plugin](github-plugin.md).

## Package manifest

Each shipped plugin declares its label, version, capability ID, description, tools, input schemas, effects, settings, views and requested storage or Vault keys in a versioned JSON manifest. The built-in Telegram channel follows the same contract. The [PayPal manifest](../crates/lifeos-paypal/manifest.json) is a complete package with read tools and Vault-backed settings.

Register each implementation with `Tool::handler` and bind the definition with `bind_manifest(include_str!("../manifest.json"))`. The SDK checks the declarations against the compiled tool names, effects and input types. For a settings field, use the corresponding name from `vars`. A variable with `vault: true` resolves from the same-named Vault key. Secret fields require Vault storage. A UI slot can reference a bundled asset. Storage can be `vault_database` or `signal_history`.

To stage a package, put its binary and declaration in `.lifeos/plugin-packages/lifeos-<id>` and `.lifeos/plugin-packages/lifeos-<id>.manifest.json`. The capability must match the ID. Plugins shows the declared access before installation. Installing pins the binary, verifies its reported manifest against the staged one and records enabled functions and Vault key names. Existing installations keep their owner-selected grants.

The [package JSON Schema](plugin-manifest.schema.json) supports editor validation. The running server publishes the same schema at `/api/plugin-manifest.schema.json` and the declaration of each enabled plugin at `/api/plugins/{id}/manifest`. These endpoints contain credential names only.

### Crew and Automation

A manifest can contribute native Crew members through `agents` and native routines through `automations`. A routine names a local agent from the same package. It supports manual runs, an optional five-field local cron schedule and an `on` list of LifeOS write-tool names or exact plugin event keys. Set `paused: true` to let the owner decide when it starts. Runs appear in Automation history. Plugin events use the durable delivery queue and a stable delivery receipt.

Declare outgoing event names and descriptions in `emits`. Declare webhook routes in `webhooks`, with a processor name and the headers it consumes. LifeOS lists those events under Automation → Signals and binds the declared webhooks when installing the package.

Each contribution has a stable ID, `plugin.<package>.<local-id>`. Use `lifeosTools` to request native tools for a Crew member, such as reading tasks or logging a habit. Crew restrictions still apply. Agents receive only the declared `pluginTools` that are also enabled by the owner, and Vault resolves keys using that agent’s grant. Disabling the package withdraws its agents and routines, closes their sessions and preserves run history. The [GitHub package](../crates/lifeos-github/manifest.json) ships a reviewer and an on-demand digest as examples.

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

Restart the LifeOS server to apply changes. Remove the entry and restart to uninstall. Optional `args` are passed to the executable. Use `vars` for nonsecret configuration. Declare credential names with `vault: true` in the package manifest and save their values in the LifeOS Vault. Functions, hooks and event handlers are enabled by the owner's config. Keep the server on the private network used for LifeOS.

## Backend

- `GET /api/plugins` lists installed plugins and their enabled functions, frontend slots and startup errors. Each entry includes `retrying_deliveries`, the number of outstanding subscriber deliveries that have failed at least once.
- `POST /api/plugins/{id}` accepts the JSON string `"install"`, `"enable"` or `"disable"` and the header `X-LifeOS-Plugin: 1`. Install is available for staged packages in the catalog.
- `POST /api/plugins/{id}/functions/{tool}` calls a manifest tool with a body such as `{"arguments":{}}`.
- `POST /api/plugins/{id}/hooks/{name}` invokes the configured processor with a JSON object as its parameters. For the example, send `{"message":"Hello"}` to `/api/plugins/hello/hooks/echo`.
- A hook can instead use an object binding with `processor` and `headers`. The processor receives `{"body":"…","headers":{…}}` with the original UTF-8 body and only the named lowercase headers. This allows a plugin to verify a signed request before processing it. The body limit is 2 MiB.
- Event handlers receive the event in `ProcessInput.reading` and an empty object in `params`. `lifeos.started` fires when the server starts. `lifeos.changed` carries the current web revision and can coalesce rapid changes. A processor can emit readings under `plugin.<id>.<keySuffix>`; a configured handler can subscribe to these keys. LifeOS commits emitted events and subscriber deliveries to `.lifeos/plugin-events.sqlite` before acknowledging the hook. HTTP success means the hook processor finished and its events were accepted. Subscribers run asynchronously. A nonempty `Reading.dedupe` suppresses repeat events with the same full key and token for 30 days. Each subscriber receives a stable `request_id` across retries and service restarts. Use this ID for side effects in external systems. Failed deliveries retry with exponential backoff up to 256 seconds between attempts. The worker skips bindings removed or disabled before delivery. Each chain allows up to 64 events and eight levels.

## Agent tools over MCP

Declare each operation with SDK `Tool::new` and add its name to the installed plugin’s `functions` allowlist. LifeOS publishes the manifest description, JSON input schema and read-only annotation over MCP. A plugin can consist entirely of tools.

The Assistant connects to enabled plugins when its session starts. For example, it can call `mcp__plugin_car__list_expenses` for a month, then `save_expense` with the returned ID and edited record. Omit the ID to create a record, or use `delete_expense` with an ID to remove it. Crew members do not receive these servers automatically.

Other MCP clients can launch `lifeos mcp --plugin car` over stdio. Use `--home` for a non-default vault. Every list and call checks the current registry. Reconnect after replacing a binary.

## Frontend

Declare a `page` slot for Integrations → Plugins or a `dashboard` slot for Today. Set `expose` to an HTML or JSON asset path, or leave it empty to use `Ui.entry`. HTML slots run in a sandboxed iframe. JSON slots use LifeOS components. The asset endpoint uses the MIME type supplied by the plugin, falling back to the filename extension for an empty type. Invalid MIME declarations return HTTP 502. From the iframe, send `{type: "lifeos:call", id, name, arguments}` to `parent.postMessage`. The host answers with `{type: "lifeos:answer", id, result}` or `error`. The host also sends `{type: "lifeos:event", name, revision}` for `lifeos.ready` and `lifeos.changed`. The [example](../crates/plugin-sdk/examples/hello.rs) demonstrates these entry points.

LifeOS implements the host bindings above. Remit's Module Federation UI, agent grants, credential proxy, account and forge services, and automatic poll and sensor scheduling require separate host integrations.

A slot that should fit its content can send `{type: "lifeos:resize", height}` with the measured content height in CSS pixels. The host caps the requested content height at 1600 pixels. A `ResizeObserver` on the content can report changes when text wraps.

Declare a `sidebar` slot with a UI asset, a unique slot ID, and a title to add a navigation item on desktop and mobile. LifeOS opens the asset at `/plugins/{plugin-id}/{slot-id}` using the same renderer as dashboard slots.

The optional **Chi tiêu ô tô** package (`lifeos-car`) records fuel, maintenance and other vehicle expenses in VND. Install it under Integrations → Plugins after staging the binary in the plugin-packages directory. Its MCP tools use the `plugin_car_expenses` table in the vault database. On first launch the updated plugin imports rows and IDs from the former `car-expenses.sqlite` file.

### Native JSON collections

Expose a JSON asset from any UI slot. The version 1 `collection` renderer supports a month filter, a grouped currency summary, a field-driven form and editable history. A dashboard slot renders the summary. The plugin supplies labels, field definitions and the three tool names. Read receives the month parameter and returns a serialized array of records. Save receives the record fields and a nullable ID, and remove receives its ID. The owner enables those tools in the existing function allowlist. See the [example collection](plugin-collection.example.json) for optional and conditional inputs.

### Shared database tables

Trusted Rust plugins can opt into the SDK's `storage` Cargo feature. Include `"{lifeos_db}"` as an item in the installed plugin's `args` to receive the vault database path at launch. Pass it, the plugin ID, and the latest schema version to `plugin_sdk::storage::open`. The SDK runs the callback for each pending version and commits the migrations and per-plugin version together. It returns a SQLite connection with foreign keys enabled. Use a plugin-specific prefix for tables and indexes. This grants the trusted binary direct read/write access to the vault database.

```rust
let db = plugin_sdk::storage::open(path, "mileage", 1, |tx, version| {
    if version == 1 {
        tx.execute_batch("CREATE TABLE plugin_mileage_trips(id INTEGER PRIMARY KEY, km INTEGER NOT NULL)")?;
    }
    Ok(())
})?;
```

### PayPal

Open PayPal in Connections, select Sandbox or Live, and save `PAYPAL_CLIENT_ID` and `PAYPAL_CLIENT_SECRET` in Vault. Share both keys with Assistant. Ask for balances or transactions over up to 31 calendar days. The plugin settings retain the environment. Vault owns credential rotation and removal.

The PayPal plugin reports total and available balances by currency. See the [Balances API reference](https://developer.paypal.com/docs/api/transaction-search/v1/#balances_get).

### Component trees

A JSON slot can expose a component tree directly. To build the tree on each visit, expose a descriptor such as `{"kind":"ui_source","tool":"overview_ui","arguments":{}}`. The enabled read function returns the serialized tree. Agents use the same document as the `content` argument to `save_widget`. Call `list_ui_components` to discover export names. The [catalog](../web/src/components/ui-catalog.json) maps each name to its shadcn-vue family.

```json
{
  "kind": "ui",
  "root": "panel",
  "elements": {
    "panel": {"component":"shadcn","name":"Card","slots":{"default":["header","body"]}},
    "header": {"component":"shadcn","name":"CardHeader","slots":{"default":["title"]}},
    "title": {"component":"heading","text":"Weekly activity"},
    "body": {"component":"shadcn","name":"CardContent","slots":{"default":["chart"]}},
    "chart": {"component":"shadcn","name":"Graph","props":{"part":{"id":"weekly","label":"Sessions","kind":"bar","x":["Mon","Tue"],"series":[{"name":"Sessions","values":[3,5]}],"unit":null}}}
  }
}
```

Use ordered child IDs in each named Vue slot. Give every node a unique ID and a single parent, except the root. Use uncontrolled input props such as `defaultValue` and `defaultOpen` for local interactions. Compose triggers and content beneath their provider, for example `TabsList` and `TabsContent` under `Tabs`. A `button` node with `label` and `prompt` opens an Assistant draft. Graphs accept multiple series with one numeric value per axis label, and support `bar` and `line`.

For relationships use `NetworkGraph` with a text `label`, `nodes` of shape `{id,label}`, and `edges` of shape `{source,target}` referencing node IDs.


### Plugin settings manifest

The host renders **Details** from the plugin label, capability, processors, enabled tools and UI slots. Page slots open at `/plugins/{id}/{slot-id}`. **Settings** opens the host form at `/connections/plugins/{id}/settings`.

Declare a UI slot such as `{"slot":"settings","id":"settings","title":"Settings","expose":"settings.json"}` and bundle that JSON asset. The [version 1 schema](plugin-settings.schema.json) defines field order, labels, required fields, text and password inputs, select options and defaults. Each key must be unique and match a manifest variable. A secret variable must use the secret control. A select default must be one of its options. Without a settings slot, LifeOS uses the plugin's declared variables to build the form.

The owner settings endpoint validates updates server-side, preserves omitted fields and stores the plugin variables atomically. Saved secret fields return a presence indicator. Leaving a password input empty when editing preserves its saved value. Clearing settings removes only the fields declared by that form. The [PayPal manifest](../crates/lifeos-paypal/manifest.json) embeds its settings definition.

A prop value such as `{"$slot":"item.value"}` reads a value supplied by a parent Vue slot. Calendar inputs accept ISO dates. In a conversation, agents can use `show_block` with a part of shape `{"ui":{"id":"overview","document":...}}` to show the tree inline.
