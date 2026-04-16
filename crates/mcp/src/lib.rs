use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sourcebot_core::{
    GlobTool, GrepTool, ListReposTool, ListTreeTool, ReadFileTool, RetrievalTool,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{retrieval_tool_definitions, server_manifest};

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
}
