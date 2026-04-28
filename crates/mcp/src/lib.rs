use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sourcebot_core::{
    BlobStore, CatalogStore, GlobStore, GlobTool, GrepStore, GrepTool, ListReposTool, ListTreeTool,
    ReadFileTool, RetrievalTool, RetrievalToolContext, TreeStore,
};

pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerManifest {
    pub protocol_version: String,
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerCapabilities {
    pub tools: ToolCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCapabilities {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub fn server_manifest() -> ServerManifest {
    ServerManifest {
        protocol_version: MCP_PROTOCOL_VERSION.into(),
        server_info: ServerInfo {
            name: env!("CARGO_PKG_NAME").into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        capabilities: ServerCapabilities {
            tools: ToolCapabilities {
                list_changed: false,
            },
        },
    }
}

pub fn retrieval_tool_definitions() -> Vec<McpToolDefinition> {
    vec![
        mcp_tool_definition(ListReposTool, object_schema([], [])),
        mcp_tool_definition(
            ListTreeTool::default(),
            object_schema([("path", string_schema())], []),
        ),
        mcp_tool_definition(
            ReadFileTool::new("path"),
            object_schema([("path", string_schema())], ["path"]),
        ),
        mcp_tool_definition(
            GlobTool::new("pattern"),
            object_schema([("pattern", string_schema())], ["pattern"]),
        ),
        mcp_tool_definition(
            GrepTool::new("query"),
            object_schema([("query", string_schema())], ["query"]),
        ),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolCallRequest {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallErrorCode {
    UnknownTool,
    InvalidArguments,
    PermissionDenied,
    ExecutionFailed,
    SerializationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolCallError {
    pub code: McpToolCallErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum McpToolCallResponse {
    Success {
        tool_name: String,
        structured_content: Value,
    },
    Error {
        tool_name: String,
        error: McpToolCallError,
    },
}

impl McpToolCallResponse {
    pub fn success(tool_name: impl Into<String>, structured_content: Value) -> Self {
        Self::Success {
            tool_name: tool_name.into(),
            structured_content,
        }
    }

    pub fn error(
        tool_name: impl Into<String>,
        code: McpToolCallErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self::Error {
            tool_name: tool_name.into(),
            error: McpToolCallError {
                code,
                message: message.into(),
            },
        }
    }

    pub fn tool_name(&self) -> &str {
        match self {
            Self::Success { tool_name, .. } | Self::Error { tool_name, .. } => tool_name,
        }
    }

    pub fn error_code(&self) -> Option<McpToolCallErrorCode> {
        match self {
            Self::Success { .. } => None,
            Self::Error { error, .. } => Some(error.code.clone()),
        }
    }

    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Success { .. } => None,
            Self::Error { error, .. } => Some(error.message.as_str()),
        }
    }
}

pub async fn execute_tool_call(
    catalog: &dyn CatalogStore,
    trees: &dyn TreeStore,
    blobs: &dyn BlobStore,
    globs: &dyn GlobStore,
    greps: &dyn GrepStore,
    context: &RetrievalToolContext,
    request: McpToolCallRequest,
) -> McpToolCallResponse {
    let tool_name = request.name;
    let result = match tool_name.as_str() {
        "list_repos" => {
            execute_retrieval_tool(
                tool_name.as_str(),
                request.arguments,
                |_args: ListReposArguments| Ok(ListReposTool),
                catalog,
                trees,
                blobs,
                globs,
                greps,
                context,
            )
            .await
        }
        "list_tree" => {
            execute_retrieval_tool(
                tool_name.as_str(),
                request.arguments,
                |args: ListTreeArguments| Ok(ListTreeTool::new(args.path)),
                catalog,
                trees,
                blobs,
                globs,
                greps,
                context,
            )
            .await
        }
        "read_file" => {
            execute_retrieval_tool(
                tool_name.as_str(),
                request.arguments,
                |args: ReadFileArguments| Ok(ReadFileTool::new(args.path)),
                catalog,
                trees,
                blobs,
                globs,
                greps,
                context,
            )
            .await
        }
        "glob" => {
            execute_retrieval_tool(
                tool_name.as_str(),
                request.arguments,
                |args: GlobArguments| Ok(GlobTool::new(args.pattern)),
                catalog,
                trees,
                blobs,
                globs,
                greps,
                context,
            )
            .await
        }
        "grep" => {
            execute_retrieval_tool(
                tool_name.as_str(),
                request.arguments,
                |args: GrepArguments| Ok(GrepTool::new(args.query)),
                catalog,
                trees,
                blobs,
                globs,
                greps,
                context,
            )
            .await
        }
        _ => Err(McpToolCallResponse::error(
            tool_name.as_str(),
            McpToolCallErrorCode::UnknownTool,
            format!("unknown MCP tool: {tool_name}"),
        )),
    };

    match result {
        Ok(response) | Err(response) => response,
    }
}

fn mcp_tool_definition(tool: impl RetrievalTool, input_schema: Value) -> McpToolDefinition {
    let definition = tool.definition();
    McpToolDefinition {
        name: definition.name,
        description: definition.description,
        input_schema,
    }
}

fn string_schema() -> Value {
    json!({ "type": "string" })
}

fn object_schema<const P: usize, const R: usize>(
    properties: [(&str, Value); P],
    required: [&str; R],
) -> Value {
    let mut schema = Map::from_iter([("type".into(), Value::String("object".into()))]);
    schema.insert(
        "properties".into(),
        Value::Object(Map::from_iter(
            properties
                .into_iter()
                .map(|(name, value)| (name.to_string(), value)),
        )),
    );

    if !required.is_empty() {
        schema.insert(
            "required".into(),
            Value::Array(
                required
                    .into_iter()
                    .map(|name| Value::String(name.to_string()))
                    .collect(),
            ),
        );
    }

    Value::Object(schema)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListReposArguments {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListTreeArguments {
    #[serde(default)]
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadFileArguments {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobArguments {
    pattern: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrepArguments {
    query: String,
}

async fn execute_retrieval_tool<TArgs, TTool, TBuild>(
    tool_name: &str,
    arguments: Value,
    build_tool: TBuild,
    catalog: &dyn CatalogStore,
    trees: &dyn TreeStore,
    blobs: &dyn BlobStore,
    globs: &dyn GlobStore,
    greps: &dyn GrepStore,
    context: &RetrievalToolContext,
) -> Result<McpToolCallResponse, McpToolCallResponse>
where
    TArgs: DeserializeOwned,
    TTool: RetrievalTool,
    TBuild: FnOnce(TArgs) -> Result<TTool, McpToolCallResponse>,
{
    let parsed_arguments = parse_tool_arguments::<TArgs>(tool_name, arguments)?;
    let tool = build_tool(parsed_arguments)?;
    let result = tool
        .run(catalog, trees, blobs, globs, greps, context)
        .await
        .map_err(|err| {
            let message = err.to_string();
            McpToolCallResponse::error(tool_name, classify_execution_error(&message), message)
        })?;

    let structured_content = serde_json::to_value(&result).map_err(|err| {
        McpToolCallResponse::error(
            tool_name,
            McpToolCallErrorCode::SerializationFailed,
            err.to_string(),
        )
    })?;

    Ok(McpToolCallResponse::success(tool_name, structured_content))
}

fn parse_tool_arguments<TArgs>(
    tool_name: &str,
    arguments: Value,
) -> Result<TArgs, McpToolCallResponse>
where
    TArgs: DeserializeOwned,
{
    if !arguments.is_object() {
        return Err(McpToolCallResponse::error(
            tool_name,
            McpToolCallErrorCode::InvalidArguments,
            format!("invalid arguments for tool {tool_name}: expected a JSON object"),
        ));
    }

    serde_json::from_value(arguments).map_err(|err| {
        McpToolCallResponse::error(
            tool_name,
            McpToolCallErrorCode::InvalidArguments,
            format!("invalid arguments for tool {tool_name}: {err}"),
        )
    })
}

fn classify_execution_error(message: &str) -> McpToolCallErrorCode {
    if message.contains(" is not visible to the retrieval context")
        || message.contains(" is outside retrieval scope")
    {
        McpToolCallErrorCode::PermissionDenied
    } else {
        McpToolCallErrorCode::ExecutionFailed
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::json;
    use sourcebot_core::{
        BlobStore, CatalogStore, GlobStore, GrepStore, ImportRepositoryResult, RepositoryBlob,
        RepositoryTree, RepositoryTreeEntry, RepositoryTreeEntryKind, RetrievalToolContext,
        TreeStore,
    };
    use sourcebot_models::{Connection, RepositoryDetail, RepositorySummary, SyncState};

    use crate::{
        execute_tool_call, retrieval_tool_definitions, server_manifest, McpToolCallErrorCode,
        McpToolCallRequest, McpToolCallResponse,
    };

    struct StaticCatalogStore {
        repositories: Vec<RepositorySummary>,
    }

    struct NullTreeStore;
    struct NullBlobStore;
    struct TripwireBlobStore;
    struct NullGlobStore;
    struct NullGrepStore;

    struct StaticTreeStore {
        tree: Option<RepositoryTree>,
    }

    #[async_trait]
    impl CatalogStore for StaticCatalogStore {
        async fn list_repositories(&self) -> anyhow::Result<Vec<RepositorySummary>> {
            Ok(self.repositories.clone())
        }

        async fn get_repository_detail(
            &self,
            _repo_id: &str,
        ) -> anyhow::Result<Option<RepositoryDetail>> {
            Ok(None)
        }

        async fn import_local_repository(
            &self,
            _connection: Connection,
            _repo_path: &str,
        ) -> anyhow::Result<ImportRepositoryResult> {
            anyhow::bail!("local repository import unsupported by MCP static catalog test store")
        }
    }

    #[async_trait]
    impl TreeStore for NullTreeStore {
        async fn get_tree(
            &self,
            _repo_id: &str,
            _path: &str,
        ) -> anyhow::Result<Option<RepositoryTree>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl BlobStore for NullBlobStore {
        async fn get_blob(
            &self,
            _repo_id: &str,
            _path: &str,
        ) -> anyhow::Result<Option<RepositoryBlob>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl BlobStore for TripwireBlobStore {
        async fn get_blob(
            &self,
            repo_id: &str,
            path: &str,
        ) -> anyhow::Result<Option<RepositoryBlob>> {
            panic!("read_file widened into blob store for repo {repo_id} at path {path}");
        }
    }

    #[async_trait]
    impl GlobStore for NullGlobStore {
        async fn glob_paths(
            &self,
            _repo_id: &str,
            _pattern: &str,
        ) -> anyhow::Result<Option<sourcebot_core::RepositoryGlob>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl GrepStore for NullGrepStore {
        async fn grep(
            &self,
            _repo_id: &str,
            _query: &str,
        ) -> anyhow::Result<Option<sourcebot_core::RepositoryGrep>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl TreeStore for StaticTreeStore {
        async fn get_tree(
            &self,
            _repo_id: &str,
            _path: &str,
        ) -> anyhow::Result<Option<RepositoryTree>> {
            Ok(self.tree.clone())
        }
    }

    #[test]
    fn server_manifest_serializes_expected_mcp_metadata() {
        let value = serde_json::to_value(server_manifest()).expect("manifest should serialize");

        assert_eq!(
            value,
            json!({
                "protocol_version": "2025-06-18",
                "server_info": {
                    "name": "sourcebot-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": {
                    "tools": {
                        "list_changed": false,
                    }
                }
            })
        );
    }

    #[test]
    fn retrieval_tool_definitions_expose_expected_tools_in_stable_order() {
        let tools = retrieval_tool_definitions();
        let names = tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec!["list_repos", "list_tree", "read_file", "glob", "grep"]
        );
    }

    #[test]
    fn retrieval_tool_definitions_expose_expected_input_schemas() {
        let tools = retrieval_tool_definitions();

        assert_eq!(
            tools[0].input_schema,
            json!({
                "type": "object",
                "properties": {},
            })
        );
        assert_eq!(
            tools[1].input_schema,
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
            })
        );
        assert_eq!(
            tools[2].input_schema,
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"],
            })
        );
        assert_eq!(
            tools[3].input_schema,
            json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" }
                },
                "required": ["pattern"],
            })
        );
        assert_eq!(
            tools[4].input_schema,
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"],
            })
        );
    }

    #[tokio::test]
    async fn execute_tool_call_dispatches_list_repos_and_serializes_structured_output() {
        let response = execute_tool_call(
            &StaticCatalogStore {
                repositories: vec![
                    RepositorySummary {
                        id: "repo_sourcebot_rewrite".into(),
                        name: "sourcebot-rewrite".into(),
                        default_branch: "main".into(),
                        sync_state: SyncState::Ready,
                    },
                    RepositorySummary {
                        id: "repo_secret".into(),
                        name: "secret".into(),
                        default_branch: "main".into(),
                        sync_state: SyncState::Ready,
                    },
                ],
            },
            &NullTreeStore,
            &NullBlobStore,
            &NullGlobStore,
            &NullGrepStore,
            &RetrievalToolContext {
                active_repo_id: Some("repo_sourcebot_rewrite".into()),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
            },
            McpToolCallRequest {
                name: "list_repos".into(),
                arguments: json!({}),
            },
        )
        .await;

        assert_eq!(
            response,
            McpToolCallResponse::success(
                "list_repos",
                json!({
                    "tool": "list_repos",
                    "payload": {
                        "repositories": [
                            {
                                "id": "repo_sourcebot_rewrite",
                                "name": "sourcebot-rewrite",
                                "default_branch": "main",
                                "sync_state": "ready",
                            }
                        ]
                    }
                })
            )
        );
    }

    #[tokio::test]
    async fn execute_tool_call_dispatches_list_tree_for_active_repo_scope() {
        let response = execute_tool_call(
            &StaticCatalogStore {
                repositories: vec![],
            },
            &StaticTreeStore {
                tree: Some(RepositoryTree {
                    repo_id: "repo_sourcebot_rewrite".into(),
                    path: "src".into(),
                    entries: vec![RepositoryTreeEntry {
                        name: "lib.rs".into(),
                        path: "src/lib.rs".into(),
                        kind: RepositoryTreeEntryKind::File,
                    }],
                }),
            },
            &NullBlobStore,
            &NullGlobStore,
            &NullGrepStore,
            &RetrievalToolContext {
                active_repo_id: Some("repo_sourcebot_rewrite".into()),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
            },
            McpToolCallRequest {
                name: "list_tree".into(),
                arguments: json!({ "path": "src" }),
            },
        )
        .await;

        assert_eq!(
            response,
            McpToolCallResponse::success(
                "list_tree",
                json!({
                    "tool": "list_tree",
                    "payload": {
                        "repo_id": "repo_sourcebot_rewrite",
                        "path": "src",
                        "entries": [
                            {
                                "name": "lib.rs",
                                "path": "src/lib.rs",
                                "kind": "file",
                            }
                        ]
                    }
                })
            )
        );
    }

    #[tokio::test]
    async fn execute_tool_call_read_file_denies_out_of_scope_active_repo_before_blob_lookup() {
        let response = execute_tool_call(
            &StaticCatalogStore {
                repositories: vec![],
            },
            &NullTreeStore,
            &TripwireBlobStore,
            &NullGlobStore,
            &NullGrepStore,
            &RetrievalToolContext {
                active_repo_id: Some("repo_secret".into()),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                visible_repo_ids: vec!["repo_sourcebot_rewrite".into(), "repo_secret".into()],
            },
            McpToolCallRequest {
                name: "read_file".into(),
                arguments: json!({ "path": "secrets.txt" }),
            },
        )
        .await;

        assert_eq!(response.tool_name(), "read_file");
        assert_eq!(
            response.error_code(),
            Some(McpToolCallErrorCode::PermissionDenied)
        );
        assert_eq!(
            response.error_message(),
            Some("active repository repo_secret is outside retrieval scope")
        );
        assert_eq!(
            serde_json::to_value(response).expect("MCP response should serialize"),
            json!({
                "status": "error",
                "tool_name": "read_file",
                "error": {
                    "code": "permission_denied",
                    "message": "active repository repo_secret is outside retrieval scope",
                }
            })
        );
    }

    #[tokio::test]
    async fn execute_tool_call_rejects_unknown_tool_names() {
        let response = execute_tool_call(
            &StaticCatalogStore {
                repositories: vec![],
            },
            &NullTreeStore,
            &NullBlobStore,
            &NullGlobStore,
            &NullGrepStore,
            &RetrievalToolContext::default(),
            McpToolCallRequest {
                name: "unknown_tool".into(),
                arguments: json!({}),
            },
        )
        .await;

        assert_eq!(
            response,
            McpToolCallResponse::error(
                "unknown_tool",
                McpToolCallErrorCode::UnknownTool,
                "unknown MCP tool: unknown_tool"
            )
        );
    }

    #[tokio::test]
    async fn execute_tool_call_rejects_missing_required_arguments() {
        let response = execute_tool_call(
            &StaticCatalogStore {
                repositories: vec![],
            },
            &NullTreeStore,
            &NullBlobStore,
            &NullGlobStore,
            &NullGrepStore,
            &RetrievalToolContext {
                active_repo_id: Some("repo_sourcebot_rewrite".into()),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
            },
            McpToolCallRequest {
                name: "read_file".into(),
                arguments: json!({}),
            },
        )
        .await;

        assert_eq!(
            response.error_code(),
            Some(McpToolCallErrorCode::InvalidArguments)
        );
        assert_eq!(response.tool_name(), "read_file");
        assert!(response
            .error_message()
            .expect("error response")
            .contains("missing field `path`"));
    }

    #[tokio::test]
    async fn execute_tool_call_rejects_non_object_arguments() {
        let response = execute_tool_call(
            &StaticCatalogStore {
                repositories: vec![],
            },
            &NullTreeStore,
            &NullBlobStore,
            &NullGlobStore,
            &NullGrepStore,
            &RetrievalToolContext::default(),
            McpToolCallRequest {
                name: "list_tree".into(),
                arguments: json!("src"),
            },
        )
        .await;

        assert_eq!(
            response.error_code(),
            Some(McpToolCallErrorCode::InvalidArguments)
        );
        assert!(response
            .error_message()
            .expect("error response")
            .contains("expected a JSON object"));
    }
}
