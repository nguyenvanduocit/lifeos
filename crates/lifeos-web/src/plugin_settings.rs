//! A settings slot references a versioned JSON asset. Older plugins use their manifest vars.
use super::*;
use std::collections::BTreeSet;

use plugin_wire::plugin::{Field, Kind, SettingsSchema};

fn invalid_schema() -> ApiError {
    (
        StatusCode::BAD_GATEWAY,
        "Invalid plugin settings schema".into(),
    )
}

async fn schema(plugin: &Installed) -> Result<SettingsSchema, ApiError> {
    let manifest = plugin.manifest().await?;
    if let Some(settings) = &manifest.settings {
        return Ok(settings.clone());
    }
    if let Some(slot) = manifest
        .ui
        .iter()
        .flat_map(|ui| &ui.slots)
        .find(|s| s.slot == "settings")
    {
        if !asset_path(&slot.expose) || !slot.expose.ends_with(".json") {
            return Err(invalid_schema());
        }
        let asset = plugin
            .remote
            .asset(slot.expose.clone())
            .await
            .map_err(failed)?;
        let spec: SettingsSchema =
            serde_json::from_slice(&asset.content).map_err(|_| invalid_schema())?;
        let mut keys = BTreeSet::new();
        if spec.version != 1
            || spec.fields.is_empty()
            || spec.fields.iter().any(|f| {
                !keys.insert(&f.key)
                    || f.label.trim().is_empty()
                    || !manifest
                        .vars
                        .iter()
                        .any(|v| v.name == f.key && (!v.secret || matches!(f.kind, Kind::Secret)))
                    || (matches!(f.kind, Kind::Select)
                        && (f.options.is_empty()
                            || (!f.default.is_empty() && !f.options.contains(&f.default))))
                    || (matches!(f.kind, Kind::Secret) && !f.default.is_empty())
            })
        {
            return Err(invalid_schema());
        }
        return Ok(spec);
    }
    Ok(SettingsSchema {
        version: 1,
        description: String::new(),
        fields: manifest
            .vars
            .iter()
            .map(|v| Field {
                key: v.name.clone(),
                label: v.what.clone(),
                kind: if v.secret { Kind::Secret } else { Kind::Text },
                required: !v.optional,
                options: vec![],
                default: String::new(),
            })
            .collect(),
    })
}

fn owner(headers: &HeaderMap) -> Result<(), ApiError> {
    crate::accounts::owner_request(headers)
        .then_some(())
        .ok_or((StatusCode::FORBIDDEN, "Open settings in LifeOS".into()))
}

fn telegram_schema() -> SettingsSchema {
    lifeos_mcp::telegram::definition()
        .manifest()
        .settings
        .expect("Telegram settings")
}

pub(crate) async fn settings_view(
    State(state): State<Arc<HttpState>>,
    Route(id): Route<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    owner(&headers)?;
    let (spec, vars, vault_keys) = if id == "telegram" {
        let config = state.app.telegram.current();
        if !config.installed || !config.enabled {
            return Err(not_found("Telegram is unavailable"));
        }
        let mut vars = Vars::new();
        vars.insert(
            lifeos_mcp::telegram::CHATS.into(),
            config
                .chats
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        );
        if state.app.telegram.configured() {
            vars.insert(lifeos_mcp::telegram::TOKEN.into(), "*".into());
        }
        (
            telegram_schema(),
            vars,
            vec!["TELEGRAM_BOT_TOKEN".to_owned()],
        )
    } else {
        let plugin = state.app.plugins.enabled(&id)?;
        (
            schema(&plugin).await?,
            plugin.vars()?,
            plugin.config.vault_keys.clone(),
        )
    };
    let mut values = Vars::new();
    let mut saved = Vec::new();
    for field in &spec.fields {
        if let Some(value) = vars.get(&field.key).filter(|v| !v.is_empty()) {
            saved.push(&field.key);
            if !matches!(field.kind, Kind::Secret) && !vault_keys.contains(&field.key) {
                values.insert(field.key.clone(), value.clone());
            }
        }
    }
    Ok(crate::accounts::private_response(
        StatusCode::OK,
        json!({"schema":spec,"values":values,"saved":saved,"vault_keys":vault_keys,"can_reset":id!="telegram"}),
    ))
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SettingsChange {
    Save { values: Vars },
    Reset,
}

pub(crate) async fn settings_save(
    State(state): State<Arc<HttpState>>,
    Route(id): Route<String>,
    headers: HeaderMap,
    Json(input): Json<SettingsChange>,
) -> Result<StatusCode, ApiError> {
    owner(&headers)?;
    if id == "telegram" {
        let SettingsChange::Save { values } = input else {
            return Err((StatusCode::BAD_REQUEST, "Reset unavailable".into()));
        };
        if values.keys().any(|k| {
            k != "token"
                && k != "chats"
                && k != lifeos_mcp::telegram::TOKEN
                && k != lifeos_mcp::telegram::CHATS
        }) {
            return Err((StatusCode::BAD_REQUEST, "Unknown setting".into()));
        }
        let raw = values
            .get(lifeos_mcp::telegram::CHATS)
            .or_else(|| values.get("chats"))
            .map(String::as_str)
            .unwrap_or("")
            .trim();
        let chats = if raw.is_empty() {
            vec![]
        } else {
            raw.split(',')
                .map(|id| id.trim().parse())
                .collect::<Result<Vec<i64>, _>>()
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        "Enter numeric chat IDs separated by commas".into(),
                    )
                })?
        };
        state.app.telegram.save(crate::telegram::Settings {
            token: values
                .get(lifeos_mcp::telegram::TOKEN)
                .or_else(|| values.get("token"))
                .cloned()
                .unwrap_or_default(),
            chats,
        })?;
    } else {
        let plugins = &state.app.plugins;
        let _guard = plugins.update.lock().await;
        let old = plugins.enabled(&id)?;
        let spec = schema(&old).await?;
        let mut entries = configs(&plugins.path).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot read settings".into(),
            )
        })?;
        let config = entries
            .get_mut(&id)
            .filter(|c| !c.disabled)
            .ok_or_else(|| not_found("Plugin unavailable"))?;
        let mut credentials = Vars::new();
        match input {
            SettingsChange::Reset => {
                for field in spec.fields {
                    config.vars.remove(&field.key);
                }
            }
            SettingsChange::Save { values } => {
                if values
                    .keys()
                    .any(|k| !spec.fields.iter().any(|f| &f.key == k))
                {
                    return Err((StatusCode::BAD_REQUEST, "Unknown setting".into()));
                }
                for field in spec.fields {
                    if config.vault_keys.contains(&field.key) {
                        if let Some(value) = values.get(&field.key).filter(|v| !v.is_empty()) {
                            credentials.insert(field.key, value.clone());
                        }
                        continue;
                    }
                    if let Some(value) = values.get(&field.key) {
                        config.vars.insert(field.key.clone(), value.clone());
                    }
                    let value = config
                        .vars
                        .get(&field.key)
                        .map(String::as_str)
                        .unwrap_or(&field.default);
                    if (field.required && value.trim().is_empty())
                        || (!value.is_empty()
                            && matches!(field.kind, Kind::Select)
                            && !field.options.iter().any(|o| o == value))
                        || value.contains(['\r', '\n', '\0'])
                    {
                        return Err((
                            StatusCode::BAD_REQUEST,
                            format!("Invalid value for {}", field.label),
                        ));
                    }
                    if !value.is_empty() && !config.vars.contains_key(&field.key) {
                        config.vars.insert(field.key, field.default);
                    }
                }
            }
        }
        for (key, value) in credentials {
            state
                .app
                .life
                .lock()
                .expect("life lock")
                .set_vault_key(&key, &value)
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Cannot save {key} to the Vault"),
                    )
                })?;
        }
        let replacement = Arc::new(Installed::new(&id, config.clone(), &plugins.path));
        plugins.persist_configs(&entries).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot save settings".into(),
            )
        })?;
        plugins
            .entries
            .write()
            .expect("plugin registry lock")
            .insert(id.clone(), replacement);
        old.remote.retire().await;
    }
    state.app.bump();
    Ok(StatusCode::NO_CONTENT)
}
