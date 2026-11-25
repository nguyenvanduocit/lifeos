//! SQLite at `<home>/.lifeos/lifeos.db`.
//!
//! Two roles, kept apart:
//! - **Index** (`docs`, `docs_fts`): derived from the Markdown vault, safe to
//!   drop and rebuild with `lifeos reindex`.
//! - **Records** (`habits`, `habit_logs`, `interactions`, `transactions`,
//!   `budgets`, `metrics`, `inbox`, `kv`): data whose source of truth is this
//!   database. Back it up.

use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // v1
    r#"
    CREATE TABLE docs(
        path    TEXT PRIMARY KEY,
        kind    TEXT NOT NULL,
        id      TEXT NOT NULL,
        title   TEXT NOT NULL,
        status  TEXT,
        due     TEXT,
        goal    TEXT,
        horizon TEXT,
        tags    TEXT NOT NULL DEFAULT '',
        mtime   INTEGER NOT NULL
    );
    CREATE INDEX docs_kind_status ON docs(kind, status);
    CREATE VIRTUAL TABLE docs_fts USING fts5(
        path UNINDEXED, title, body,
        tokenize = 'unicode61 remove_diacritics 2'
    );

    CREATE TABLE habit_logs(
        habit TEXT NOT NULL,
        day   TEXT NOT NULL,
        value REAL NOT NULL DEFAULT 1,
        note  TEXT,
        PRIMARY KEY(habit, day)
    );

    -- amount_minor: signed, in the currency's minor unit (VND: đồng, USD: cents).
    -- Negative = money out, positive = money in.
    CREATE TABLE transactions(
        id           INTEGER PRIMARY KEY,
        day          TEXT NOT NULL,
        amount_minor INTEGER NOT NULL,
        currency     TEXT NOT NULL,
        category     TEXT NOT NULL,
        note         TEXT
    );
    CREATE INDEX transactions_day ON transactions(day);

    CREATE TABLE metrics(
        id    INTEGER PRIMARY KEY,
        day   TEXT NOT NULL,
        name  TEXT NOT NULL,
        value REAL NOT NULL,
        unit  TEXT,
        note  TEXT
    );
    CREATE INDEX metrics_name_day ON metrics(name, day);

    CREATE TABLE kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    "#,
    // v2: people, habit definitions, budgets, inbox; the index carries the new
    // frontmatter fields (it is derived: dropped here, rebuilt on open).
    r#"
    DROP TABLE docs;
    DROP TABLE docs_fts;
    CREATE TABLE docs(
        path          TEXT PRIMARY KEY,
        kind          TEXT NOT NULL,
        id            TEXT NOT NULL,
        title         TEXT NOT NULL,
        status        TEXT,
        due           TEXT,
        scheduled     TEXT,
        priority      TEXT,
        goal          TEXT,
        horizon       TEXT,
        progress      INTEGER,
        relation      TEXT,
        birthday      TEXT,
        contact_every INTEGER,
        completed     TEXT,
        tags          TEXT NOT NULL DEFAULT '',
        mtime         INTEGER NOT NULL
    );
    CREATE INDEX docs_kind_status ON docs(kind, status);
    CREATE INDEX docs_kind_id ON docs(kind, id);
    CREATE VIRTUAL TABLE docs_fts USING fts5(
        path UNINDEXED, title, body,
        tokenize = 'unicode61 remove_diacritics 2'
    );

    CREATE TABLE habits(
        name            TEXT PRIMARY KEY,
        unit            TEXT,
        target_per_week INTEGER NOT NULL DEFAULT 7,
        created         TEXT NOT NULL,
        archived        INTEGER NOT NULL DEFAULT 0
    );
    INSERT INTO habits(name, created) SELECT habit, MIN(day) FROM habit_logs GROUP BY habit;

    -- person: the id (file stem) of a document in people/.
    CREATE TABLE interactions(
        id      INTEGER PRIMARY KEY,
        person  TEXT NOT NULL,
        day     TEXT NOT NULL,
        channel TEXT,
        note    TEXT
    );
    CREATE INDEX interactions_person_day ON interactions(person, day);

    CREATE TABLE budgets(
        category      TEXT NOT NULL,
        currency      TEXT NOT NULL,
        monthly_minor INTEGER NOT NULL,
        PRIMARY KEY(category, currency)
    );

    CREATE TABLE inbox(
        id      INTEGER PRIMARY KEY,
        created TEXT NOT NULL,
        text    TEXT NOT NULL,
        done    INTEGER NOT NULL DEFAULT 0
    );
    "#,
    // v3: what agents remember, and every conversation turn (so a restart
    // loses neither).
    r#"
    CREATE TABLE memories(
        id      INTEGER PRIMARY KEY,
        holder  TEXT NOT NULL,
        kind    TEXT NOT NULL,
        title   TEXT NOT NULL,
        body    TEXT NOT NULL,
        created TEXT NOT NULL,
        updated TEXT NOT NULL,
        ended   TEXT,
        why_ended TEXT,
        recalled INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX memories_holder ON memories(holder, ended);
    CREATE VIRTUAL TABLE memories_fts USING fts5(title, body, tokenize = 'unicode61 remove_diacritics 2');
    CREATE TABLE turns(
        id    INTEGER PRIMARY KEY,
        agent TEXT NOT NULL,
        at    TEXT NOT NULL,
        turn  TEXT NOT NULL
    );
    CREATE INDEX turns_agent ON turns(agent, id);
    "#,
    // v4: routine runs, what the write tools did (routines run after them),
    // and outside MCP servers.
    r#"
    CREATE TABLE runs(
        id       INTEGER PRIMARY KEY,
        routine  TEXT NOT NULL,
        title    TEXT NOT NULL,
        agent    TEXT NOT NULL,
        trigger  TEXT NOT NULL,
        detail   TEXT,
        payload  TEXT,
        queued   TEXT NOT NULL,
        started  TEXT,
        finished TEXT,
        ms       INTEGER,
        error    TEXT,
        reply    TEXT
    );
    CREATE INDEX runs_routine ON runs(routine, id);
    CREATE INDEX runs_waiting ON runs(started) WHERE started IS NULL;
    CREATE TABLE events(
        id      INTEGER PRIMARY KEY,
        at      TEXT NOT NULL,
        caller  TEXT NOT NULL,
        routine TEXT,
        tool    TEXT NOT NULL,
        input   TEXT NOT NULL,
        output  TEXT NOT NULL
    );
    CREATE TABLE connections(
        name    TEXT PRIMARY KEY,
        preset  TEXT,
        url     TEXT,
        command TEXT,
        args    TEXT NOT NULL DEFAULT '[]',
        env     TEXT NOT NULL DEFAULT '{}',
        headers TEXT NOT NULL DEFAULT '{}',
        enabled INTEGER NOT NULL DEFAULT 1,
        created TEXT NOT NULL
    );
    "#,
    // v5: durable schedule cursors, run snapshots, and hashed webhook keys.
    r#"
    ALTER TABLE runs ADD COLUMN instructions TEXT NOT NULL DEFAULT '';
    CREATE TABLE routine_schedules(
        routine TEXT PRIMARY KEY,
        expression TEXT NOT NULL,
        next INTEGER
    );
    CREATE TABLE routine_hooks(
        routine TEXT PRIMARY KEY,
        digest BLOB NOT NULL
    );
    "#,
    // v6: account metadata and runtime health. Credentials live in the
    // isolated Claude profiles, outside the vault.
    r#"
    CREATE TABLE agent_accounts (
        id TEXT PRIMARY KEY,
        label TEXT NOT NULL,
        kind TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'unchecked',
        reason TEXT,
        quota TEXT,
        utilization REAL,
        resets_at INTEGER,
        retry_at INTEGER
    );
    INSERT INTO agent_accounts(id,label,kind) VALUES ('default','Tài khoản của máy','default');
    "#,
    // v7: remit's backbone (docs/adr/0001). Each agent works on a card; its
    // conversation is the card's stream of events, which is also its outbox
    // (queued prompts). The write tools' log keeps its rows as `tool_log`.
    // Turns kept before become a prompt, a message and a stop each.
    r#"
    ALTER TABLE events RENAME TO tool_log;
    CREATE TABLE cards(
        id         TEXT PRIMARY KEY,
        doc        TEXT NOT NULL
    );
    CREATE TABLE events(
        id           INTEGER PRIMARY KEY,
        card         TEXT NOT NULL,
        kind         TEXT NOT NULL,
        status       TEXT NOT NULL DEFAULT '',
        tool_call_id TEXT NOT NULL DEFAULT '',
        tool_kind    TEXT NOT NULL DEFAULT '',
        tool_name    TEXT NOT NULL DEFAULT '',
        title        TEXT NOT NULL DEFAULT '',
        text         TEXT NOT NULL DEFAULT '',
        by_actor     TEXT,
        priority     INTEGER NOT NULL DEFAULT 0,
        data         TEXT NOT NULL DEFAULT '',
        created      TEXT NOT NULL,
        updated      TEXT NOT NULL,
        rev          INTEGER NOT NULL DEFAULT 1
    );
    CREATE INDEX events_card ON events(card, id);
    CREATE INDEX events_queued ON events(card, priority DESC, id) WHERE kind = 'prompt' AND status = 'queued';
    CREATE INDEX events_tool ON events(card, tool_call_id) WHERE kind = 'tool';
    CREATE TABLE drafts(
        card TEXT PRIMARY KEY,
        text TEXT NOT NULL,
        rev  INTEGER NOT NULL
    );
    INSERT INTO events(card, kind, status, text, by_actor, data, created, updated)
        SELECT card, kind, status, text, by_actor, data, at, at FROM (
            SELECT id AS tid, 0 AS seq, agent AS card, 'prompt' AS kind, 'delivered' AS status,
                   json_extract(turn, '$.ask') AS text,
                   CASE WHEN json_extract(turn, '$.source') IS NULL
                        THEN '{"kind":"owner","id":"","name":"You"}'
                        ELSE json_object('kind', 'routine', 'id', '', 'name', json_extract(turn, '$.source')) END
                       AS by_actor,
                   '' AS data, replace(at, ' ', 'T') || ':00' AS at
            FROM turns
            UNION ALL
            SELECT id, 1, agent, 'message', '',
                   COALESCE((SELECT group_concat(json_extract(value, '$.Text'), '') FROM json_each(turn, '$.reply.steps')
                             WHERE json_extract(value, '$.Text') IS NOT NULL), ''),
                   NULL, '', replace(at, ' ', 'T') || ':01'
            FROM turns
            UNION ALL
            SELECT id, 2, agent, 'stop', 'end_turn', '', NULL,
                   json_object('ms', json_extract(turn, '$.reply.ms')), replace(at, ' ', 'T') || ':02'
            FROM turns
        ) ORDER BY tid, seq;
    DELETE FROM events WHERE kind = 'message' AND text = '';
    DROP TABLE turns;
    DELETE FROM kv WHERE key LIKE 'chat.reset.%' OR key LIKE 'chat.session.%';
    "#,
    // v8: questions agents put to the owner, which the Assistant decides for him first (remit's ADR-0087)
    r#"
    CREATE TABLE questions(
        id         INTEGER PRIMARY KEY,
        asker      TEXT NOT NULL,
        asker_name TEXT NOT NULL,
        text       TEXT NOT NULL,
        status     TEXT NOT NULL DEFAULT 'open',
        answer     TEXT NOT NULL DEFAULT '',
        why        TEXT NOT NULL DEFAULT '',
        decided_by TEXT,
        overturned TEXT NOT NULL DEFAULT '',
        created    TEXT NOT NULL,
        decided    TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX questions_open ON questions(status, id);
    "#,
    // v9: memory as remit keeps it (docs/adr/0001, remit's ADR-0117): how far a person stands behind each memory, its
    // use counted beside it, the vectors it is searched by in meaning, and its links to other memories.
    r#"
    ALTER TABLE memories ADD COLUMN trust TEXT NOT NULL DEFAULT 'auto';
    CREATE TABLE memory_use(
        memory        INTEGER PRIMARY KEY,
        helped        INTEGER NOT NULL DEFAULT 0,
        misled        INTEGER NOT NULL DEFAULT 0,
        last_recalled TEXT
    );
    CREATE TABLE memory_embeddings(
        memory INTEGER NOT NULL,
        model  TEXT NOT NULL,
        hash   TEXT NOT NULL,
        vector BLOB NOT NULL,
        PRIMARY KEY(memory, model)
    );
    CREATE TABLE memory_unembedded(
        memory INTEGER NOT NULL,
        model  TEXT NOT NULL,
        hash   TEXT NOT NULL,
        until  TEXT NOT NULL,
        PRIMARY KEY(memory, model)
    );
    CREATE TABLE memory_links(
        src  INTEGER NOT NULL,
        dst  INTEGER NOT NULL,
        kind TEXT NOT NULL,
        PRIMARY KEY(src, dst, kind)
    );
    CREATE INDEX memory_links_dst ON memory_links(dst);
    "#,
    // v10: the librarian writes memory from teachings (remit's ADR-0117): what was taught, to whom, and what it wrote.
    r#"
    ALTER TABLE memories ADD COLUMN source INTEGER;
    ALTER TABLE memories ADD COLUMN evidence TEXT NOT NULL DEFAULT '';
    CREATE TABLE teachings(
        id         INTEGER PRIMARY KEY,
        holder     TEXT NOT NULL,
        card       TEXT NOT NULL DEFAULT '',
        role       TEXT NOT NULL,
        source     TEXT NOT NULL,
        teacher    TEXT NOT NULL DEFAULT '',
        from_event INTEGER NOT NULL DEFAULT 0,
        to_event   INTEGER NOT NULL DEFAULT 0,
        text       TEXT NOT NULL DEFAULT '',
        status     TEXT NOT NULL DEFAULT 'open',
        note       TEXT NOT NULL DEFAULT '',
        written    TEXT NOT NULL DEFAULT '[]',
        created    TEXT NOT NULL,
        settled    TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX teachings_open ON teachings(status, id);
    "#,
    // v11: workflow runs and the journal of their steps (remit's ADR-0073): a run is its script re-evaluated against
    // the results its calls have had, in the order they arrived.
    r#"
    CREATE TABLE workflow_runs(
        id          INTEGER PRIMARY KEY,
        name        TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        script      TEXT NOT NULL,
        args        TEXT NOT NULL DEFAULT '',
        status      TEXT NOT NULL DEFAULT 'running',
        result      TEXT NOT NULL DEFAULT '',
        error       TEXT NOT NULL DEFAULT '',
        started_by  TEXT NOT NULL DEFAULT '',
        created     TEXT NOT NULL,
        finished    TEXT NOT NULL DEFAULT ''
    );
    CREATE TABLE workflow_steps(
        run      INTEGER NOT NULL,
        seq      INTEGER NOT NULL,
        verb     TEXT NOT NULL,
        label    TEXT NOT NULL DEFAULT '',
        phase    TEXT NOT NULL DEFAULT '',
        input    TEXT NOT NULL DEFAULT '',
        hash     TEXT NOT NULL,
        status   TEXT NOT NULL DEFAULT 'pending',
        result   TEXT NOT NULL DEFAULT '',
        error    TEXT NOT NULL DEFAULT '',
        arrived  INTEGER,
        due      TEXT NOT NULL DEFAULT '',
        created  TEXT NOT NULL,
        finished TEXT NOT NULL DEFAULT '',
        PRIMARY KEY(run, seq)
    );
    CREATE INDEX workflow_steps_due ON workflow_steps(status, due) WHERE verb = 'wait';
    "#,
    // v12: what each card's session was given, loaded and holds (remit's session_launches, session_inits,
    // session_contexts), each the newest of its card, as JSON of its lifeos-wire type.
    r#"
    CREATE TABLE session_launches(
        card       TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        resumed    INTEGER NOT NULL DEFAULT 0,
        at         TEXT NOT NULL,
        given      TEXT NOT NULL
    );
    CREATE TABLE session_inits(
        card       TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        doc        TEXT NOT NULL
    );
    CREATE TABLE session_contexts(
        card       TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        doc        TEXT NOT NULL
    );
    "#,
    // v13: the images the owner puts in a message to an agent (remit's ADR-0111), stored once by the id the page made,
    // apart from the vault's documents; a draft and the prompt it becomes name them (`ImageRef`, JSON).
    r#"
    CREATE TABLE images(
        id         TEXT PRIMARY KEY,
        media_type TEXT NOT NULL,
        bytes      BLOB NOT NULL,
        at         TEXT NOT NULL
    );
    ALTER TABLE drafts ADD COLUMN images TEXT NOT NULL DEFAULT '[]';
    "#,
    // v14: what agents ask and hand each other (docs/adr/0002): a row until what came of it went back to the sender.
    // `batch` groups what one piece of a sender's work handed out, which comes back together; `prompt` is the target's
    // prompt event it was queued as; `seen` marks news the target has read.
    r#"
    CREATE TABLE handoffs(
        id          INTEGER PRIMARY KEY,
        kind        TEXT NOT NULL,
        sender      TEXT NOT NULL,
        sender_name TEXT NOT NULL,
        target      TEXT NOT NULL,
        target_name TEXT NOT NULL,
        text        TEXT NOT NULL,
        status      TEXT NOT NULL DEFAULT 'queued',
        result      TEXT NOT NULL DEFAULT '',
        parent      INTEGER,
        batch       TEXT NOT NULL,
        chain       TEXT NOT NULL DEFAULT '[]',
        prompt      INTEGER NOT NULL DEFAULT 0,
        attempts    INTEGER NOT NULL DEFAULT 1,
        note        TEXT NOT NULL DEFAULT '',
        seen        INTEGER NOT NULL DEFAULT 0,
        created     TEXT NOT NULL,
        updated     TEXT NOT NULL,
        closed      TEXT NOT NULL DEFAULT '',
        reported    INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX handoffs_open ON handoffs(status, id);
    CREATE INDEX handoffs_batch ON handoffs(batch, reported);
    CREATE INDEX handoffs_sender ON handoffs(sender, id);
    CREATE INDEX handoffs_parent ON handoffs(parent);
    "#,
    // v15: measurements a device sends (Apple Health from the iPhone app) carry their source; sending a day again
    // replaces that source's value instead of adding a row.
    r#"
    ALTER TABLE metrics ADD COLUMN source TEXT;
    CREATE UNIQUE INDEX metrics_source_name_day ON metrics(source, name, day) WHERE source IS NOT NULL;
    "#,
    // v16: the Macs running LifeOS Computer (desktop use), kept as each last said, online while connected.
    r#"
    CREATE TABLE computers(
        id               TEXT PRIMARY KEY,
        name             TEXT NOT NULL,
        version          TEXT NOT NULL DEFAULT '',
        accessibility    INTEGER NOT NULL DEFAULT 0,
        screen_recording INTEGER NOT NULL DEFAULT 0,
        width            INTEGER NOT NULL DEFAULT 0,
        height           INTEGER NOT NULL DEFAULT 0,
        paused           INTEGER NOT NULL DEFAULT 0,
        online           INTEGER NOT NULL DEFAULT 0,
        seen             TEXT NOT NULL
    );
    "#,
    // v17: the Vault (after agentes.chat's ADR 0144): logins, keys and contacts, and who may use each. A password's or
    // a key's value is never here: it sits in the private secrets directory, one file per item (`item-<id>`).
    r#"
    CREATE TABLE vault_items(
        id          INTEGER PRIMARY KEY,
        kind        TEXT NOT NULL CHECK (kind IN ('login','key','contact')),
        title       TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        website     TEXT,
        username    TEXT,
        key_name    TEXT UNIQUE,
        key_hint    TEXT,
        contact     TEXT,
        all_agents  INTEGER NOT NULL DEFAULT 0,
        agents      TEXT NOT NULL DEFAULT '[]',
        last_used   TEXT,
        created     TEXT NOT NULL,
        updated     TEXT NOT NULL
    );
    CREATE TABLE vault_uses(
        id     INTEGER PRIMARY KEY,
        item   INTEGER NOT NULL REFERENCES vault_items(id) ON DELETE CASCADE,
        action TEXT NOT NULL,
        agent  TEXT,
        detail TEXT NOT NULL DEFAULT '',
        at     TEXT NOT NULL
    );
    CREATE INDEX vault_uses_item ON vault_uses(item, at);
    "#,
    r#"
    CREATE TABLE custom_widgets(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        page TEXT NOT NULL,
        slot TEXT NOT NULL,
        title TEXT NOT NULL,
        markdown TEXT NOT NULL,
        agent TEXT NOT NULL
    );
    CREATE INDEX custom_widgets_page ON custom_widgets(page, id);
    "#,
    "UPDATE custom_widgets SET slot = 'top' WHERE slot <> 'top';",
    r#"
    ALTER TABLE custom_widgets ADD COLUMN content TEXT;
    ALTER TABLE custom_widgets ADD COLUMN prompt TEXT NOT NULL DEFAULT '';
    ALTER TABLE custom_widgets ADD COLUMN size TEXT NOT NULL DEFAULT 'm' CHECK(size IN ('s', 'm', 'l'));
    ALTER TABLE custom_widgets ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE custom_widgets ADD COLUMN version INTEGER NOT NULL DEFAULT 0;
    UPDATE custom_widgets SET position = id;
    "#,
    r#"
    ALTER TABLE custom_widgets ADD COLUMN grid_x INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE custom_widgets ADD COLUMN grid_y INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE custom_widgets ADD COLUMN grid_w INTEGER NOT NULL DEFAULT 6;
    ALTER TABLE custom_widgets ADD COLUMN grid_h INTEGER NOT NULL DEFAULT 2;
    WITH ordered AS (
        SELECT id, ROW_NUMBER() OVER (PARTITION BY page ORDER BY position, id) - 1 AS n,
               CASE size WHEN 's' THEN 3 ELSE 6 END AS w,
               CASE size WHEN 'l' THEN 4 ELSE 2 END AS h
        FROM custom_widgets
    ), bands AS (
        SELECT (n / 2) AS band, custom_widgets.page, MAX(h) AS height
        FROM ordered JOIN custom_widgets USING (id)
        GROUP BY custom_widgets.page, n / 2
    ), offsets AS (
        SELECT band, page, COALESCE(SUM(height) OVER (
            PARTITION BY page ORDER BY band ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
        ), 0) AS y
        FROM bands
    )
    UPDATE custom_widgets AS widget
    SET grid_x = (ordered.n % 2) * 6, grid_y = offsets.y, grid_w = ordered.w, grid_h = ordered.h
    FROM ordered JOIN offsets ON offsets.band = ordered.n / 2
    WHERE widget.id = ordered.id AND widget.page = offsets.page;
    "#,
    "UPDATE custom_widgets SET grid_x = grid_x * 4, grid_y = grid_y * 4,
                            grid_w = grid_w * 4, grid_h = grid_h * 4;",
    r#"
    ALTER TABLE habits ADD COLUMN entity_id TEXT;
    UPDATE habits SET entity_id = 'habit_' || lower(hex(randomblob(12)));
    CREATE UNIQUE INDEX habits_entity_id ON habits(entity_id);
    CREATE TABLE habit_aliases (
        name TEXT PRIMARY KEY,
        entity_id TEXT NOT NULL REFERENCES habits(entity_id)
    );
    CREATE TRIGGER habits_record_rename AFTER UPDATE OF name ON habits
    WHEN OLD.name <> NEW.name BEGIN
        INSERT INTO habit_aliases (name, entity_id) VALUES (OLD.name, OLD.entity_id)
            ON CONFLICT(name) DO NOTHING;
        UPDATE habit_logs SET habit = NEW.name WHERE habit = OLD.name;
    END;
    "#,
    // Keep published row IDs reserved after deletion, including across a restart.
    r#"
    CREATE TABLE retired_row_ids (table_name TEXT PRIMARY KEY, highest INTEGER NOT NULL);
    CREATE TRIGGER interactions_reserve_deleted_id AFTER DELETE ON interactions BEGIN
        INSERT INTO retired_row_ids VALUES ('interactions', OLD.id)
        ON CONFLICT(table_name) DO UPDATE SET highest = MAX(highest, excluded.highest);
    END;
    CREATE TRIGGER transactions_reserve_deleted_id AFTER DELETE ON transactions BEGIN
        INSERT INTO retired_row_ids VALUES ('transactions', OLD.id)
        ON CONFLICT(table_name) DO UPDATE SET highest = MAX(highest, excluded.highest);
    END;
    CREATE TRIGGER metrics_reserve_deleted_id AFTER DELETE ON metrics BEGIN
        INSERT INTO retired_row_ids VALUES ('metrics', OLD.id)
        ON CONFLICT(table_name) DO UPDATE SET highest = MAX(highest, excluded.highest);
    END;
    CREATE TRIGGER vault_items_reserve_deleted_id AFTER DELETE ON vault_items BEGIN
        INSERT INTO retired_row_ids VALUES ('vault_items', OLD.id)
        ON CONFLICT(table_name) DO UPDATE SET highest = MAX(highest, excluded.highest);
    END;
    "#,
];

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as i64 + 1)?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_v3_turning_conversations_into_streams() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join(".lifeos");
        std::fs::create_dir(&state).unwrap();
        let old = Connection::open(state.join("lifeos.db")).unwrap();
        for sql in &MIGRATIONS[..3] { old.execute_batch(sql).unwrap(); }
        old.pragma_update(None, "user_version", 3).unwrap();
        old.execute(
            "INSERT INTO turns(agent,at,turn) VALUES ('assistant','2026-01-02 08:00',?1)",
            [r#"{"ask":"Chào","at":"2026-01-02 08:00","reply":{"steps":[],"ms":2,"cost_usd":null,"error":null}}"#],
        ).unwrap();
        drop(old);

        let life = crate::Life::open(dir.path()).unwrap();
        let stream = life.events("assistant", None, 10).unwrap();
        let said: Vec<_> = stream.iter().map(|e| (e.kind, e.text.as_str())).collect();
        assert_eq!(said, [(lifeos_wire::EventKind::Prompt, "Chào"), (lifeos_wire::EventKind::Stop, "")]);
        assert_eq!(stream[0].by.as_ref().map(|a| a.kind), Some(lifeos_wire::ActorKind::Owner));
        assert_eq!(life.agent_accounts().unwrap()[0].id, "default");
        assert!(life.routines().unwrap().is_empty());
        assert!(dir.path().join("routines").is_dir());
    }
}
