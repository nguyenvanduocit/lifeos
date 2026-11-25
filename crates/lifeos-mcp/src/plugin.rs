//! Expose one installed plugin's owner-enabled functions as an MCP server.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use lifeos_core::plugins::{Install, configs};
use plugin::{RemotePlugin, Spec};
use plugin_sdk::{Effect, Manifest, Plugin};
use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    model::*,
    service::{RequestContext, RoleServer},
};

#[derive(Clone)]
pub struct Server {
    principal: String,
    path: PathBuf,
    id: String,
    spec: Option<Spec>,
    plugin: Arc<dyn Plugin>,
    manifest: Arc<Manifest>,
}

fn spec(path: &Path, id: &str, install: &Install) -> Spec {
    Spec {
        id: id.into(),
        binary: install.binary.clone(),
        sha256: install.sha256.clone(),
        args: install.resolved_args(path),
    }
}

fn installed(path: &Path, id: &str) -> Result<Install> {
    configs(path)?
        .remove(id)
        .filter(|i| !i.disabled)
        .context("Plugin is unavailable")
}

impl Server {
    pub fn for_agent(mut self, principal: String) -> Self {self.principal = principal; self}

    pub fn available(&self) -> bool {
        self.current().is_ok()
    }

    fn current(&self) -> Result<(Vec<String>, plugin_sdk::Vars), ErrorData> {
        let home = self.path.parent().unwrap().parent().unwrap();
        let unavailable = || ErrorData::invalid_request("Plugin is unavailable", None);
        if self.id == "telegram" {
            let c = crate::telegram::channel(home).map_err(|_| unavailable())?;
            if self.principal != lifeos_core::life::ASSISTANT || !c.installed || !c.enabled {
                return Err(unavailable());
            }
            let token = lifeos_core::Life::plugin_store(home)
                .and_then(|l| {
                    l.integration_key(crate::telegram::TOKEN, Some(&self.principal))
                })
                .map_err(|_| unavailable())?;
            return Ok((
                self.manifest.tools.iter().map(|t| t.name.clone()).collect(),
                [
                    (crate::telegram::TOKEN.into(), token.unwrap_or_default()),
                    (
                        crate::telegram::CHATS.into(),
                        c.chats
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                ]
                .into(),
            ));
        }
        let install = installed(&self.path, &self.id).map_err(|_| unavailable())?;
        if self.spec.as_ref() != Some(&spec(&self.path, &self.id, &install)) {
            return Err(unavailable());
        }
        let vars =
            lifeos_core::plugins::runtime_vars(home, &install, Some(&self.principal))
                .map_err(|_| unavailable())?;
        let functions = if self.principal == lifeos_core::life::ASSISTANT { install.functions } else {
            let agent = install.agent(&self.id,&self.principal).ok_or_else(unavailable)?;
            install.functions.iter().filter(|f|agent.plugin_tools.contains(f)).cloned().collect()
        };
        Ok((functions, vars))
    }
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(format!("plugin_{}", self.id), env!("CARGO_PKG_VERSION")),
        )
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let (functions, _) = self.current()?;
        let mut tools = Vec::new();
        for t in &self.manifest.tools {
            if functions.contains(&t.name) {
                let schema = t
                    .input_schema
                    .0
                    .as_object()
                    .ok_or_else(|| ErrorData::internal_error("Invalid plugin schema", None))?;
                tools.push(
                    Tool::new(
                        t.name.clone(),
                        t.description.clone(),
                        Arc::new(schema.clone()),
                    )
                    .with_annotations(ToolAnnotations::new().read_only(t.effect == Effect::Read)),
                );
            }
        }
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let (functions, vars) = self.current()?;
        let name = req.name.into_owned();
        if !functions.contains(&name) || !self.manifest.tools.iter().any(|t| t.name == name) {
            return Err(ErrorData::invalid_params("Unknown plugin function", None));
        }
        let request = link::Request {
            id: uuid::Uuid::new_v4().to_string(),
        };
        let result = self
            .plugin
            .call(
                request,
                name,
                req.arguments.unwrap_or_default().into(),
                vars,
            )
            .await;
        Ok(match result {
            Ok(value) => CallToolResult::success(vec![ContentBlock::text(value)]),
            Err(err) => CallToolResult::error(vec![ContentBlock::text(err.message)]),
        }
        .into())
    }
}

pub async fn open(home: &Path, id: String) -> Result<Server> {
    let path = home.join(".lifeos/plugins.json");
    let (spec, plugin): (_, Arc<dyn Plugin>) = if id == "telegram" {
        let c = crate::telegram::channel(home)?;
        anyhow::ensure!(c.installed && c.enabled, "Telegram unavailable");
        (None, Arc::new(crate::telegram::definition()))
    } else {
        let spec = spec(&path, &id, &installed(&path, &id)?);
        (Some(spec.clone()), Arc::new(RemotePlugin::new(spec)))
    };
    let manifest = plugin
        .describe()
        .await
        .map_err(|err| anyhow::anyhow!(err.message))?;
    Ok(Server {
        principal: lifeos_core::life::ASSISTANT.into(),
        path,
        id,
        spec,
        plugin,
        manifest: Arc::new(manifest),
    })
}

pub async fn serve(home: &Path, id: String) -> Result<()> {
    open(home, id)
        .await?
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
