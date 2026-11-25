use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, ensure};
use rusqlite::{Row, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Life;

const COLUMNS: &str = "id, page, slot, title, markdown, agent, content, prompt, version, grid_x, grid_y, grid_w, grid_h";

const LEGACY_HOME_SLOTS: &[&str] = &[
    "tasks",
    "brief",
    "week",
    "habits",
    "money",
    "people",
    "health",
    "stat-tasks",
    "stat-habits",
    "stat-money",
    "stat-inbox",
];

input! {
    pub struct ListWidgets {
        /// Exact page path, such as /, /tasks, or /tasks/my-task.
        pub page: String,
    }
}

input! {
    pub struct SaveWidget {
        /// Omit to create a widget. Pass an existing widget ID to update or move it.
        pub id: Option<i32>,
        /// Required on creation. Omit on update to keep the current page. Home is /.
        pub page: Option<String>,
        /// Optional. Omit or use top to place a standalone widget on the page.
        pub slot: Option<String>,
        /// Required on creation. Supply on update only when explicitly renaming the widget.
        pub title: Option<String>,
        /// Data for a native LifeOS widget. Use kpi, goal, bars, line, list, text, or ui to compose components and named slots.
        pub content: Option<WidgetContent>,
        /// Only needed when updating an existing Markdown widget in its original format.
        pub markdown: Option<String>,
        /// Standing instructions for Update now: what to measure, the source and the time range.
        /// Kept on edits when omitted. Supply specific instructions for new widgets.
        pub prompt: Option<String>,
    }
}

input! {
    pub struct RemoveWidget {
        pub id: i32,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum WidgetContent {
    Ui(WidgetUi),
    Data(WidgetData),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum WidgetUi {
    /// Compose a widget from the LifeOS component catalog.
    Ui {
        /// ID of the first component to render.
        root: String,
        /// Components by ID. Each ID appears once in the tree. At most 256 elements and 24 levels.
        elements: BTreeMap<String, WidgetElement>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "component", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum WidgetElement {
    /// Render a named shadcn-vue export (e.g. Tabs, TabsList, TabsTrigger, TabsContent)
    /// or Graph. Props are JSON, slots map Vue slot names to ordered element IDs.
    Shadcn {
        name: String,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
        props: BTreeMap<String, serde_json::Value>,
        #[serde(default)]
        slots: BTreeMap<String, Vec<String>>,
    },
    /// A vertical group of component IDs.
    Stack {
        children: Vec<String>,
    },
    /// A horizontal group that wraps on narrow screens.
    Row {
        children: Vec<String>,
    },
    /// Responsive grid with two to four columns.
    Grid {
        columns: u8,
        children: Vec<String>,
    },
    /// Named slots: each accepts an array of component IDs. Only body is required.
    Section {
        #[serde(default)]
        header: Vec<String>,
        body: Vec<String>,
        #[serde(default)]
        actions: Vec<String>,
        #[serde(default)]
        footer: Vec<String>,
    },
    Heading {
        text: String,
    },
    Text {
        text: String,
    },
    Badge {
        text: String,
    },
    /// Reuse any native KPI, progress, chart, list or text component inline.
    Data {
        title: String,
        data: WidgetData,
    },
    /// Open a draft in the Assistant. The owner reviews and sends it.
    Button {
        label: String,
        prompt: String,
    },
}

pub fn ui_catalog() -> &'static BTreeMap<String, String> {
    static CATALOG: std::sync::LazyLock<BTreeMap<String, String>> =
        std::sync::LazyLock::new(|| serde_json::from_str(include_str!(
            "../../../../web/src/components/ui-catalog.json"
        )).expect("component catalog"));
    &CATALOG
}

impl WidgetUi {
    pub fn validate(&self) -> Result<()> {
        let Self::Ui { root, elements } = self;
        ensure!(
            (1..=256).contains(&elements.len()),
            "A UI widget needs 1–256 components"
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= 65_536,
            "UI widget exceeds 64 KiB"
        );
        let mut queue = vec![(root.clone(), 1)];
        let mut seen = HashSet::new();
        while let Some((id, depth)) = queue.pop() {
            ensure!(depth <= 24, "A UI widget may be at most 24 levels deep");
            ensure!(!id.is_empty() && id.len() <= 64, "Invalid component ID");
            ensure!(
                seen.insert(id.clone()),
                "Component {id:?} appears more than once"
            );
            let element = elements
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("Unknown component ID {id:?}"))?;
            let mut add = |children: &[String]| {
                queue.extend(children.iter().map(|child| (child.clone(), depth + 1)));
            };
            match element {
                WidgetElement::Stack { children } | WidgetElement::Row { children } => {
                    add(children)
                }
                WidgetElement::Shadcn { name, slots, .. } => {
                    ensure!(ui_catalog().contains_key(name), "Unknown UI component {name}");
                    for children in slots.values() { add(children); }
                }
                WidgetElement::Grid { columns, children } => {
                    ensure!(
                        (2..=4).contains(columns),
                        "A grid requires two to four columns"
                    );
                    add(children);
                }
                WidgetElement::Section {
                    header,
                    body,
                    actions,
                    footer,
                } => {
                    add(header);
                    add(body);
                    add(actions);
                    add(footer);
                }
                WidgetElement::Heading { text } => ensure!(short(text, 120), "Invalid heading"),
                WidgetElement::Text { text } => ensure!(short(text, 2000), "Invalid text"),
                WidgetElement::Badge { text } => ensure!(short(text, 64), "Invalid badge"),
                WidgetElement::Data { title, data } => {
                    ensure!(short(title, 120), "Invalid data title");
                    data.validate()?;
                }
                WidgetElement::Button { label, prompt } => {
                    ensure!(
                        short(label, 64) && short(prompt, 1000),
                        "Invalid widget button"
                    );
                }
            }
        }
        ensure!(
            seen.len() == elements.len(),
            "Every UI component must be reachable from root"
        );
        Ok(())
    }
}

fn short(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum WidgetData {
    /// One number with an optional unit and a short comparison or note.
    Kpi {
        value: f64,
        unit: Option<String>,
        detail: Option<String>,
    },
    /// Progress from 0 to 100 percent.
    Goal { percent: f64, label: String },
    /// One numeric value per label, up to 24 values.
    Bars {
        labels: Vec<String>,
        values: Vec<f64>,
        unit: Option<String>,
    },
    /// One numeric value per label, up to 24 values.
    Line {
        labels: Vec<String>,
        values: Vec<f64>,
        unit: Option<String>,
    },
    /// Up to eight short rows, with optional secondary text.
    List { rows: Vec<WidgetRow> },
    /// A plain text summary, up to 2000 characters.
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct WidgetRow {
    pub label: String,
    pub detail: Option<String>,
}

pub const GRID_COLUMNS: i32 = 48;
pub const GRID_ROWS: i32 = 16384;

// A new card uses the first free 24×8 rectangle aligned to either half of the board.
const FIND_SPACE: &str = "
    WITH rows(y) AS (
        SELECT 0 UNION SELECT grid_y + grid_h FROM custom_widgets WHERE page = ?1
    ), columns(x) AS (VALUES (0), (24)),
    space AS MATERIALIZED (
        SELECT x, y FROM rows CROSS JOIN columns
        WHERE y <= 16376 AND NOT EXISTS (
            SELECT 1 FROM custom_widgets
            WHERE page = ?1 AND grid_x < x + 24 AND x < grid_x + grid_w
              AND grid_y < y + 8 AND y < grid_y + grid_h
        )
        ORDER BY y, x LIMIT 1
    )";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct WidgetGrid {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl WidgetGrid {
    fn in_bounds(self) -> bool {
        self.x >= 0
            && self.y >= 0
            && (8..=GRID_COLUMNS).contains(&self.w)
            && (4..=GRID_ROWS).contains(&self.h)
            && self.x <= GRID_COLUMNS - self.w
            && self.y <= GRID_ROWS - self.h
    }

    fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetPlacement {
    pub id: i32,
    pub grid: WidgetGrid,
}

impl WidgetContent {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Ui(ui) => ui.validate(),
            Self::Data(data) => data.validate(),
        }
    }
}

impl WidgetData {
    fn validate(&self) -> Result<()> {
        let number = |value: &f64| value.is_finite() && value.abs() <= 1e15;
        let valid = match self {
            Self::Kpi {
                value,
                unit,
                detail,
            } => {
                number(value)
                    && unit.as_ref().is_none_or(|s| short(s, 32))
                    && detail.as_ref().is_none_or(|s| short(s, 240))
            }
            Self::Goal { percent, label } => (0.0..=100.0).contains(percent) && short(label, 120),
            Self::Bars {
                labels,
                values,
                unit,
            }
            | Self::Line {
                labels,
                values,
                unit,
            } => {
                (1..=24).contains(&labels.len())
                    && labels.len() == values.len()
                    && labels.iter().all(|s| short(s, 64))
                    && values.iter().all(number)
                    && unit.as_ref().is_none_or(|s| short(s, 32))
            }
            Self::List { rows } => {
                (1..=8).contains(&rows.len())
                    && rows.iter().all(|row| {
                        short(&row.label, 120) && row.detail.as_ref().is_none_or(|s| short(s, 240))
                    })
            }
            Self::Text { text } => short(text, 2000),
        };
        ensure!(
            valid,
            "Invalid widget data: check the field lengths and numeric ranges described in the schema"
        );
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CustomWidget {
    pub id: i32,
    pub page: String,
    pub slot: String,
    pub title: String,
    pub markdown: String,
    pub agent: String,
    pub content: Option<WidgetContent>,
    pub prompt: String,
    pub version: i32,
    pub grid: WidgetGrid,
}

fn widget(row: &Row<'_>) -> rusqlite::Result<CustomWidget> {
    let content: Option<String> = row.get(6)?;
    let content = content
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(err))
        })?;
    Ok(CustomWidget {
        id: row.get(0)?,
        page: row.get(1)?,
        slot: row.get(2)?,
        title: row.get(3)?,
        markdown: row.get(4)?,
        agent: row.get(5)?,
        content,
        prompt: row.get(7)?,
        version: row.get(8)?,
        grid: WidgetGrid {
            x: row.get(9)?,
            y: row.get(10)?,
            w: row.get(11)?,
            h: row.get(12)?,
        },
    })
}

impl Life {
    pub fn list_widgets(&self, page: &str) -> Result<Vec<CustomWidget>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM custom_widgets WHERE page = ?1 ORDER BY grid_y, grid_x, id"
        ))?;
        Ok(stmt
            .query_map([page], widget)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_widget(&self, input: SaveWidget, agent: &str) -> Result<CustomWidget> {
        let page = input.page.as_deref().map(|page| {
            let page = page.trim();
            let normalized = if page == "/" { page } else { page.trim_end_matches('/') };
            ensure!(
                normalized.starts_with('/')
                    && normalized.len() <= 2048
                    && !normalized.contains("//")
                    && !normalized.contains(['?', '#', '\\'])
                    && !normalized.chars().any(char::is_whitespace),
                "Use the exact LifeOS page path, for example / or /tasks"
            );
            Ok(normalized)
        }).transpose()?;
        let slot = input.slot.as_deref().unwrap_or("top");
        ensure!(
            slot == "top" || (page == Some("/") && LEGACY_HOME_SLOTS.contains(&slot)),
            "Unknown widget slot. Use top."
        );
        let title = input.title.as_deref().map(str::trim);
        ensure!(
            title.is_none_or(|title| !title.is_empty() && title.chars().count() <= 120),
            "Widget title must have 1–120 characters"
        );
        let prompt = input.prompt.as_deref().map(str::trim);
        ensure!(
            prompt.is_none_or(|p| !p.is_empty() && p.chars().count() <= 2000),
            "Widget instructions must have 1–2000 characters"
        );

        let markdown = input.markdown.as_deref().map(str::trim);
        if let Some(content) = &input.content {
            content.validate()?;
            ensure!(markdown.is_none_or(str::is_empty), "Supply content or Markdown, not both");
        } else if let Some(markdown) = markdown {
            ensure!(
                !markdown.is_empty() && markdown.chars().count() <= 8000,
                "Supply typed widget content or 1–8000 characters of legacy Markdown"
            );
        }
        let content = input
            .content
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        if let Some(id) = input.id {
            ensure!(
                page.is_some() || title.is_some() || content.is_some() || markdown.is_some() || prompt.is_some(),
                "Supply at least one widget field to update"
            );
            let current_page;
            let page = match page {
                Some(page) => page,
                None => {
                    current_page = self.widget(id)?.page;
                    &current_page
                }
            };
            let sql = format!(
                "{FIND_SPACE}
                 UPDATE custom_widgets SET
                    grid_x = CASE WHEN page = ?1 THEN grid_x ELSE (SELECT x FROM space) END,
                    grid_y = CASE WHEN page = ?1 THEN grid_y ELSE (SELECT y FROM space) END,
                    grid_w = CASE WHEN page = ?1 THEN grid_w ELSE 24 END,
                    grid_h = CASE WHEN page = ?1 THEN grid_h ELSE 8 END,
                    page = ?1, title = COALESCE(?2, title),
                    markdown = CASE WHEN ?5 IS NOT NULL THEN '' ELSE COALESCE(?3, markdown) END,
                    agent = ?4, content = CASE WHEN ?3 IS NOT NULL OR ?5 IS NOT NULL THEN ?5 ELSE content END,
                    prompt = COALESCE(?6, prompt), version = version + 1
                 WHERE id = ?7 AND (page = ?1 OR EXISTS (SELECT 1 FROM space)) RETURNING {COLUMNS}"
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let mut rows =
                stmt.query(params![page, title, markdown, agent, content, prompt, id])?;
            let result = widget(rows.next()?.ok_or_else(|| {
                anyhow::anyhow!("Widget not found or no free space on the destination grid")
            })?)?;
            rows.next()?;
            return Ok(result);
        }

        let page = page.ok_or_else(|| anyhow::anyhow!("A new widget needs a page"))?;
        let title = title.ok_or_else(|| anyhow::anyhow!("A new widget needs a title"))?;
        ensure!(content.is_some() || markdown.is_some(), "A new widget needs content");
        let markdown = markdown.unwrap_or_default();
        let sql = format!(
            "{FIND_SPACE}
             INSERT INTO custom_widgets (page, slot, title, markdown, agent, content, prompt, grid_x, grid_y, grid_w, grid_h)
             SELECT ?1, 'top', ?2, ?3, ?4, ?5, COALESCE(?6, ''), x, y, 24, 8 FROM space
             RETURNING {COLUMNS}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![page, title, markdown, agent, content, prompt])?;
        let result = widget(
            rows.next()?
                .ok_or_else(|| anyhow::anyhow!("No free space on the widget grid"))?,
        )?;
        rows.next()?;
        Ok(result)
    }

    pub fn widget(&self, id: i32) -> Result<CustomWidget> {
        Ok(self.conn.query_row(
            &format!("SELECT {COLUMNS} FROM custom_widgets WHERE id = ?1"),
            [id],
            widget,
        )?)
    }

    pub fn arrange_widgets(&self, page: &str, layout: &[WidgetPlacement]) -> Result<()> {
        let tx = self.begin_event_write()?;
        let current: i64 = tx.query_row(
            "SELECT COUNT(*) FROM custom_widgets WHERE page = ?1",
            [page],
            |row| row.get(0),
        )?;
        let mut seen = std::collections::HashSet::new();
        ensure!(
            current as usize == layout.len(),
            "The widget list changed. Reopen Organize and try again."
        );
        for (index, entry) in layout.iter().enumerate() {
            let grid = entry.grid;
            ensure!(
                seen.insert(entry.id),
                "A widget appeared more than once in the layout"
            );
            ensure!(grid.in_bounds(), "A widget is outside the grid");
            ensure!(
                !layout[..index]
                    .iter()
                    .any(|other| grid.overlaps(other.grid)),
                "Widgets cannot overlap"
            );
            ensure!(
                tx.execute(
                    "UPDATE custom_widgets SET grid_x = ?1, grid_y = ?2, grid_w = ?3, grid_h = ?4
                            WHERE page = ?5 AND id = ?6",
                    params![grid.x, grid.y, grid.w, grid.h, page, entry.id]
                )? == 1,
                "The widget list changed. Reopen Organize and try again."
            );
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove_widget(&self, id: i32) -> Result<()> {
        ensure!(
            self.conn
                .execute("DELETE FROM custom_widgets WHERE id = ?1", [id])?
                == 1,
            "Widget not found"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ui_widgets_validate_named_slots_and_reject_invalid_trees() {
        let document = json!({
            "kind": "ui",
            "root": "section",
            "elements": {
                "section": {"component": "section", "header": ["title"], "body": ["data"], "actions": ["button"]},
                "title": {"component": "heading", "text": "Daily activity"},
                "data": {"component": "data", "title": "Steps", "data": {"kind": "kpi", "value": 1500}},
                "button": {"component": "button", "label": "Explain", "prompt": "Explain the activity trend"}
            }
        });
        let content: WidgetContent = serde_json::from_value(document.clone()).unwrap();
        content.validate().unwrap();

        for invalid in [
            json!(["missing"]),
            json!(["section"]),
            json!(["title"]),
            json!([]),
        ] {
            let mut changed = document.clone();
            changed["elements"]["section"]["body"] = invalid;
            let content: WidgetContent = serde_json::from_value(changed).unwrap();
            assert!(content.validate().is_err());
        }
    }

    #[test]
    fn refresh_keeps_layout_and_standing_instructions() {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        let first = life
            .save_widget(
                serde_json::from_value(json!({
                    "page": "/", "title": "Steps", "prompt": "Total steps today",
                    "content": {"kind": "kpi", "value": 10}
                }))
                .unwrap(),
                "assistant",
            )
            .unwrap();
        let second = life
            .save_widget(
                serde_json::from_value(json!({
                    "page": "/", "title": "Legacy", "markdown": "**Previous format**"
                }))
                .unwrap(),
                "assistant",
            )
            .unwrap();
        assert_eq!(
            first.grid,
            WidgetGrid {
                x: 0,
                y: 0,
                w: 24,
                h: 8
            }
        );
        assert_eq!(
            second.grid,
            WidgetGrid {
                x: 24,
                y: 0,
                w: 24,
                h: 8
            }
        );
        let resized = WidgetGrid {
            x: 13,
            y: 17,
            w: 29,
            h: 21,
        };
        life.arrange_widgets(
            "/",
            &[
                WidgetPlacement {
                    id: second.id,
                    grid: first.grid,
                },
                WidgetPlacement {
                    id: first.id,
                    grid: resized,
                },
            ],
        )
        .unwrap();
        assert!(
            life.arrange_widgets(
                "/",
                &[
                    WidgetPlacement {
                        id: first.id,
                        grid: resized
                    },
                    WidgetPlacement {
                        id: second.id,
                        grid: resized
                    },
                ]
            )
            .is_err()
        );

        let updated = life
            .save_widget(
                serde_json::from_value(json!({
                    "id": first.id,
                    "content": {"kind": "kpi", "value": 20}
                }))
                .unwrap(),
                "assistant",
            )
            .unwrap();
        assert_eq!(updated.grid, resized);
        assert_eq!(updated.title, first.title);
        assert_eq!(updated.page, first.page);
        assert_eq!(updated.prompt, first.prompt);
        assert_eq!(updated.version, first.version + 1);
        assert!(matches!(
            updated.content,
            Some(WidgetContent::Data(WidgetData::Kpi { value: 20.0, .. }))
        ));
        let saved = life.list_widgets("/").unwrap();
        assert_eq!(saved[0].id, second.id);
        assert_eq!(saved[0].markdown, "**Previous format**");
        assert_eq!(saved[1].id, first.id);
    }
}
