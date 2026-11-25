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
