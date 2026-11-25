# Data and privacy

LifeOS stores its vault on the machine running the app. `lifeos init` creates the vault layout and a `CLAUDE.md` file where you can describe what you want the agent to know.

| Vault path | Contents |
| --- | --- |
| `CLAUDE.md` | Context and instructions for the agent |
| `tasks/`, `goals/`, `people/`, `notes/` | Tasks, goals, contact profiles, and notes |
| `journal/<year>/`, `reviews/` | Journal entries, briefs, and reviews |
| `crew/` | Specialist agent configurations |
| `routines/` | Routine definitions |
| `.lifeos/lifeos.db` | Habit logs, interactions, transactions, budgets, metrics, inbox, memory, chat history, and search index |

You can edit Markdown files in any text editor or in Obsidian. Run `lifeos reindex` to rebuild the search index from Markdown. SQLite also stores original data such as transactions and chat history, so include the entire `.lifeos/` directory in your backups.

For a consistent backup, stop any LifeOS processes that write to the vault and copy the entire directory, including hidden files. To restore a backup, point `LIFEOS_HOME` at the restored vault and check it with `lifeos today`.

When you use AI features, Claude Code processes your messages and the context the agent reads from the vault. When you use the bot, messages and replies pass through Telegram. Keep the vault and token values outside the public checkout.

## Entity IDs

Click an entity's ID to copy it for use with the Assistant. The prefix identifies its type, for example
`task_goi-me`, `note_goi-me`, or `widget_42`. Editing a title in LifeOS keeps the ID. Existing document paths and
numeric IDs remain supported. If a legacy file stem matches more than one document, use the prefixed ID or full vault
path. A habit has its own `habit_…` ID and its old name still resolves after a rename.

Markdown document IDs for existing vaults use the original file stem. If you rename a vault file outside LifeOS,
references based on its previous file name need to be updated.

## The Vault

The Vault (Integrations → Vault) keeps what agents use to act for you, after agentes.chat's Vault (its ADR 0144):

- **Logins**: a website, a username and a password.
- **Keys**: an `UPPER_SNAKE` name, a description and a value. A connection sends a key where its configuration says `${KEY_NAME}`.
- **Contacts**: name, company, tax ID, email, phone and address, which agents type in forms. A contact is not a secret.

Each item is shared with every agent (those you add later too) or with the agents you tick; the librarian never gets
one. A **saved password or key comes back to no one**, you included: the page says one is saved (a key by its last four
characters), and changing it means typing a new one. Agents look in the Vault with `vault_list`, which shows titles,
websites, usernames, key names and contacts, never a value. A connection that names a key starts only in the sessions of
agents the key is shared with, and every saved value is taken out of what agents' sessions show. Each use (a key sent
to a connection, a contact read, your changes) is recorded on the item.

What an item is and who may use it are in `.lifeos/lifeos.db` (`vault_items`, `vault_uses`). The values are one private
file per item (`item-<id>`) in the secrets directory outside the vault folder: `LIFEOS_SECRETS_DIR`, or your
configuration directory's `lifeos/secrets/<vault id>/`. Agents' sessions cannot read either. Back the secrets
directory up with the vault if you want the values back after a restore. Keys saved before the Vault (one file per
name in that directory) become keys every agent may use the first time LifeOS reads the Vault.

From the host: `lifeos vault set KEY_NAME` (the value on stdin; a new key is the Assistant's), `lifeos vault remove
KEY_NAME`, `lifeos vault list`. Import a `.env` file from the Vault page: it is read in your browser, and only the keys
you save leave it.

Not yet: typing a login into a website (LifeOS agents have no browser of their own yet, and agentes.chat types a
password only after checking the page's origin), 1Password or Bitwarden as a source, and passwords agents make.
