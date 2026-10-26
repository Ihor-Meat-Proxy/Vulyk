use rmcp::{
    handler::server::wrapper::Parameters, model::*, schemars, tool, tool_router,
    ErrorData as McpError,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::VulykClient;

#[derive(Clone)]
pub struct VulykMcp {
    client: VulykClient,
}

impl VulykMcp {
    pub fn new(client: VulykClient) -> Self {
        Self { client }
    }

    fn ok_json(value: Value) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            value.to_string(),
        )]))
    }

    fn map_err(err: anyhow::Error) -> McpError {
        McpError::internal_error(err.to_string(), None)
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProjectScoped {
    /// Project UUID
    pub project_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchCasesArgs {
    pub project_id: String,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub suite_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetCaseArgs {
    pub project_id: String,
    pub case_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateCaseArgs {
    pub project_id: String,
    pub title: String,
    /// Ignored — keys are allocated as PREFIX-N by the API.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub suite_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub precondition: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub steps: Option<Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateCaseArgs {
    pub project_id: String,
    pub case_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub precondition: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub steps: Option<Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateRunArgs {
    pub project_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunIdArgs {
    pub project_id: String,
    pub run_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IngestResultsArgs {
    pub project_id: String,
    pub run_id: String,
    pub results: Value,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AskTestsArgs {
    pub project_id: String,
    pub question: String,
    #[serde(default)]
    pub case_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListCaseResultsArgs {
    pub case_id: String,
    /// Optional project scope (uses project-scoped URL when set).
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    /// Filter: Untested | Passed | Failed | Blocked | Skipped | Retest
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateRunArgs {
    pub project_id: String,
    pub run_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[tool_router(server_handler)]
impl VulykMcp {
    #[tool(description = "Search test cases in a Vulyk project (optional full-text q)")]
    async fn search_cases(
        &self,
        Parameters(args): Parameters<SearchCasesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut path = format!("/api/v1/projects/{}/cases", args.project_id);
        let mut qs = vec![];
        if let Some(q) = args.q {
            qs.push(format!("q={}", urlencoding_lite(&q)));
        }
        if let Some(suite_id) = args.suite_id {
            qs.push(format!("suite_id={suite_id}"));
        }
        if !qs.is_empty() {
            path.push('?');
            path.push_str(&qs.join("&"));
        }
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Read a test case with steps")]
    async fn get_case(
        &self,
        Parameters(args): Parameters<GetCaseArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/cases/{}",
            args.project_id, args.case_id
        );
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Create a test case (optionally with steps)")]
    async fn create_case(
        &self,
        Parameters(args): Parameters<CreateCaseArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({
            "title": args.title,
        });
        if let Some(v) = args.suite_id {
            body["suite_id"] = json!(v);
        }
        if let Some(v) = args.description {
            body["description"] = json!(v);
        }
        if let Some(v) = args.precondition {
            body["precondition"] = json!(v);
        }
        if let Some(v) = args.priority {
            body["priority"] = json!(v);
        }
        if let Some(v) = args.steps {
            body["steps"] = v;
        }
        let path = format!("/api/v1/projects/{}/cases", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Update a test case")]
    async fn update_case(
        &self,
        Parameters(args): Parameters<UpdateCaseArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({});
        if let Some(v) = args.title {
            body["title"] = json!(v);
        }
        if let Some(v) = args.description {
            body["description"] = json!(v);
        }
        if let Some(v) = args.precondition {
            body["precondition"] = json!(v);
        }
        if let Some(v) = args.priority {
            body["priority"] = json!(v);
        }
        if let Some(v) = args.status {
            body["status"] = json!(v);
        }
        if let Some(v) = args.steps {
            body["steps"] = v;
        }
        let path = format!(
            "/api/v1/projects/{}/cases/{}",
            args.project_id, args.case_id
        );
        let value = self.client.put(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "List test runs for a project")]
    async fn list_runs(
        &self,
        Parameters(args): Parameters<ProjectScoped>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/runs", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Create a test run")]
    async fn create_run(
        &self,
        Parameters(args): Parameters<CreateRunArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/runs", args.project_id);
        let value = self
            .client
            .post(&path, json!({ "name": args.name }))
            .await
            .map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Update a test run name/status")]
    async fn update_run(
        &self,
        Parameters(args): Parameters<UpdateRunArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({});
        if let Some(v) = args.name {
            body["name"] = json!(v);
        }
        if let Some(v) = args.status {
            body["status"] = json!(v);
        }
        let path = format!("/api/v1/projects/{}/runs/{}", args.project_id, args.run_id);
        let value = self.client.put(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Trigger a test run (sets status to running)")]
    async fn trigger_run(
        &self,
        Parameters(args): Parameters<RunIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/runs/{}/trigger",
            args.project_id, args.run_id
        );
        let value = self
            .client
            .post(&path, json!({}))
            .await
            .map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Ingest bulk results into a run")]
    async fn ingest_results(
        &self,
        Parameters(args): Parameters<IngestResultsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/runs/{}/results",
            args.project_id, args.run_id
        );
        let value = self
            .client
            .post(&path, json!({ "results": args.results }))
            .await
            .map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "List results for a run")]
    async fn list_results(
        &self,
        Parameters(args): Parameters<RunIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/runs/{}/results",
            args.project_id, args.run_id
        );
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "List append-only result history for a test case across runs (filter by run, status, user; paginated). Statuses: Untested, Passed, Failed, Blocked, Skipped, Retest."
    )]
    async fn list_case_results(
        &self,
        Parameters(args): Parameters<ListCaseResultsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut path = if let Some(project_id) = args.project_id {
            format!(
                "/api/v1/projects/{}/cases/{}/results",
                project_id, args.case_id
            )
        } else {
            format!("/api/v1/cases/{}/results", args.case_id)
        };
        let mut qs = vec![];
        if let Some(v) = args.run_id {
            qs.push(format!("run_id={v}"));
        }
        if let Some(v) = args.status {
            qs.push(format!("status={}", urlencoding_lite(&v)));
        }
        if let Some(v) = args.user_id {
            qs.push(format!("user_id={v}"));
        }
        if let Some(v) = args.limit {
            qs.push(format!("limit={v}"));
        }
        if let Some(v) = args.offset {
            qs.push(format!("offset={v}"));
        }
        if !qs.is_empty() {
            path.push('?');
            path.push_str(&qs.join("&"));
        }
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Ask a natural-language question about the project's tests (LLM)")]
    async fn ask_tests(
        &self,
        Parameters(args): Parameters<AskTestsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({ "question": args.question });
        if let Some(ids) = args.case_ids {
            body["case_ids"] = json!(ids);
        }
        let path = format!("/api/v1/projects/{}/llm/ask", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }
}

fn urlencoding_lite(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
