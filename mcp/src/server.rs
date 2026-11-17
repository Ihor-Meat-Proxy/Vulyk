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
pub struct EmptyArgs {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProjectScoped {
    /// Project UUID
    pub project_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetProjectArgs {
    pub project_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchCasesArgs {
    pub project_id: String,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub suite_id: Option<String>,
    /// When true with suite_id, only cases directly in that folder.
    #[serde(default)]
    pub direct_only: Option<bool>,
    /// When true, only cases with no suite_id.
    #[serde(default)]
    pub unfiled_only: Option<bool>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
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
pub struct ListRunsArgs {
    pub project_id: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateRunArgs {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub external_ref: Option<String>,
    #[serde(default)]
    pub meta: Option<Value>,
    #[serde(default)]
    pub plan_id: Option<String>,
    #[serde(default)]
    pub suite_ids: Option<Vec<String>>,
    #[serde(default)]
    pub case_ids: Option<Vec<String>>,
    #[serde(default)]
    pub case_statuses: Option<Vec<String>>,
    #[serde(default)]
    pub tag_names: Option<Vec<String>>,
    /// When true, include every project case if no other selectors are set.
    #[serde(default)]
    pub include_all_cases: Option<bool>,
    /// Start the run immediately (status=in_progress) when creating with items.
    #[serde(default)]
    pub start: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunIdArgs {
    pub project_id: String,
    /// Run UUID, unique slug, or display ID (e.g. `VQA-R-12`).
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DispatchCiArgs {
    /// CI config UUID (from list_ci_configs / project CI settings).
    pub ci_config_id: String,
    #[serde(default)]
    pub inputs: Option<Value>,
    /// Optional Vulyk run to attach; created automatically when omitted.
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub run_name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDispatchArgs {
    pub dispatch_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenerateCasesArgs {
    pub project_id: String,
    /// Free-text requirements (used when requirement_ids is empty).
    #[serde(default)]
    pub requirements_text: Option<String>,
    #[serde(default)]
    pub requirement_ids: Option<Vec<String>>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub suite_id: Option<String>,
    /// When true, persist generated drafts as cases.
    #[serde(default)]
    pub persist: Option<bool>,
    #[serde(default)]
    pub config_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenerateCodeArgs {
    pub project_id: String,
    pub case_ids: Vec<String>,
    /// `playwright` or `pytest` (default playwright).
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub config_id: Option<String>,
    /// When the project has multiple automation repos, select which to index.
    #[serde(default)]
    pub repo_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ChatArgs {
    pub project_id: String,
    pub message: String,
    #[serde(default)]
    pub history: Option<Value>,
    #[serde(default)]
    pub config_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ChatConfirmArgs {
    pub project_id: String,
    /// Full ChatConfirmRequest body (proposal + placement options).
    pub body: Value,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListRequirementsArgs {
    pub project_id: String,
    #[serde(default)]
    pub q: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRequirementArgs {
    pub project_id: String,
    pub requirement_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateRequirementArgs {
    pub project_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub acceptance_criteria: Option<String>,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub external_key: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LinkRequirementArgs {
    pub project_id: String,
    pub case_id: String,
    pub requirement_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateCampaignArgs {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetCampaignArgs {
    pub project_id: String,
    /// Campaign UUID or display_id (PREFIX-C-n).
    pub campaign_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListIterationsArgs {
    pub project_id: String,
    #[serde(default)]
    pub campaign_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetIterationArgs {
    pub project_id: String,
    /// Iteration UUID or display_id (PREFIX-IT-n).
    pub iteration_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateIterationArgs {
    pub project_id: String,
    pub campaign_id: String,
    pub name: String,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub cadence: Option<String>,
    #[serde(default)]
    pub case_ids: Option<Vec<String>>,
    #[serde(default)]
    pub suite_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RepoIdArgs {
    pub repo_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateAutomationRepoArgs {
    pub name: String,
    pub provider: String,
    pub remote_url: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub subpath: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub actions_token: Option<String>,
    #[serde(default)]
    pub ci_workflow_file: Option<String>,
    #[serde(default)]
    pub ci_workflow_ref: Option<String>,
    #[serde(default)]
    pub poll_interval_minutes: Option<i32>,
    #[serde(default)]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportRunArgs {
    pub project_id: String,
    pub run_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportTrendsArgs {
    pub project_id: String,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[tool_router(server_handler)]
impl VulykMcp {
    #[tool(description = "List all Vulyk projects visible to the API token")]
    async fn list_projects(
        &self,
        Parameters(_args): Parameters<EmptyArgs>,
    ) -> Result<CallToolResult, McpError> {
        let value = self
            .client
            .get("/api/v1/projects")
            .await
            .map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Get a Vulyk project by UUID")]
    async fn get_project(
        &self,
        Parameters(args): Parameters<GetProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "Search/list test cases in a project. Filters: q, suite_id, direct_only, unfiled_only, tag, status, priority, sort, limit, offset."
    )]
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
        if let Some(v) = args.direct_only {
            qs.push(format!("direct_only={v}"));
        }
        if let Some(v) = args.unfiled_only {
            qs.push(format!("unfiled_only={v}"));
        }
        if let Some(tag) = args.tag {
            qs.push(format!("tag={}", urlencoding_lite(&tag)));
        }
        if let Some(status) = args.status {
            qs.push(format!("status={}", urlencoding_lite(&status)));
        }
        if let Some(priority) = args.priority {
            qs.push(format!("priority={}", urlencoding_lite(&priority)));
        }
        if let Some(sort) = args.sort {
            qs.push(format!("sort={}", urlencoding_lite(&sort)));
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

    #[tool(description = "List suites/folders for a project (with case counts)")]
    async fn list_suites(
        &self,
        Parameters(args): Parameters<ProjectScoped>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/suites", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "List test runs for a project. Optional filters: status, limit, offset.")]
    async fn list_runs(
        &self,
        Parameters(args): Parameters<ListRunsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut path = format!("/api/v1/projects/{}/runs", args.project_id);
        let mut qs = vec![];
        if let Some(status) = args.status {
            qs.push(format!("status={}", urlencoding_lite(&status)));
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

    #[tool(
        description = "Get a test run by UUID, unique slug, or display ID (e.g. PREFIX-R-12). Returns run detail with progress and items."
    )]
    async fn get_run(
        &self,
        Parameters(args): Parameters<RunIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/runs/{}", args.project_id, args.run_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "Create a test run. Accepts name plus optional description, status, suite_ids, case_ids, tag_names, include_all_cases, start, etc."
    )]
    async fn create_run(
        &self,
        Parameters(args): Parameters<CreateRunArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({ "name": args.name });
        if let Some(v) = args.description {
            body["description"] = json!(v);
        }
        if let Some(v) = args.status {
            body["status"] = json!(v);
        }
        if let Some(v) = args.source {
            body["source"] = json!(v);
        }
        if let Some(v) = args.external_ref {
            body["external_ref"] = json!(v);
        }
        if let Some(v) = args.meta {
            body["meta"] = v;
        }
        if let Some(v) = args.plan_id {
            body["plan_id"] = json!(v);
        }
        if let Some(v) = args.suite_ids {
            body["suite_ids"] = json!(v);
        }
        if let Some(v) = args.case_ids {
            body["case_ids"] = json!(v);
        }
        if let Some(v) = args.case_statuses {
            body["case_statuses"] = json!(v);
        }
        if let Some(v) = args.tag_names {
            body["tag_names"] = json!(v);
        }
        if let Some(v) = args.include_all_cases {
            body["include_all_cases"] = json!(v);
        }
        if let Some(v) = args.start {
            body["start"] = json!(v);
        }
        let path = format!("/api/v1/projects/{}/runs", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
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

    #[tool(
        description = "Mark a Vulyk test run as running (status only). Does NOT start CI — use dispatch_ci to trigger Jenkins/Forgejo/Gitea/Woodpecker/GitHub/GitLab pipelines."
    )]
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

    #[tool(description = "List CI configs for a project (needed before dispatch_ci)")]
    async fn list_ci_configs(
        &self,
        Parameters(args): Parameters<ProjectScoped>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/ci-configs", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "Dispatch a CI pipeline for a config (GitHub, GitLab, Jenkins, Forgejo/Gitea, Woodpecker). Returns a CiDispatch including external_url (build/pipeline link) when the provider supplies one."
    )]
    async fn dispatch_ci(
        &self,
        Parameters(args): Parameters<DispatchCiArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({
            "inputs": args.inputs.unwrap_or(json!({})),
        });
        if let Some(v) = args.run_id {
            body["run_id"] = json!(v);
        }
        if let Some(v) = args.run_name {
            body["run_name"] = json!(v);
        }
        let path = format!("/api/v1/ci-configs/{}/dispatch", args.ci_config_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "Get a CI dispatch by id. Response includes status, external_id, and external_url (build/pipeline link) for native providers."
    )]
    async fn get_dispatch(
        &self,
        Parameters(args): Parameters<GetDispatchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/ci-dispatches/{}", args.dispatch_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "Generate test case drafts via LLM (POST …/llm/generate-cases). Provide requirements_text and/or requirement_ids; optional count, suite_id, persist, config_id."
    )]
    async fn generate_cases(
        &self,
        Parameters(args): Parameters<GenerateCasesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({});
        if let Some(v) = args.requirements_text {
            body["requirements_text"] = json!(v);
        }
        if let Some(v) = args.requirement_ids {
            body["requirement_ids"] = json!(v);
        }
        if let Some(v) = args.count {
            body["count"] = json!(v);
        }
        if let Some(v) = args.suite_id {
            body["suite_id"] = json!(v);
        }
        if let Some(v) = args.persist {
            body["persist"] = json!(v);
        }
        if let Some(v) = args.config_id {
            body["config_id"] = json!(v);
        }
        let path = format!("/api/v1/projects/{}/llm/generate-cases", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
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

    #[tool(
        description = "Generate automation code (playwright or pytest) for case_ids via LLM. Optional repo_id when multiple automation repos are linked."
    )]
    async fn generate_code(
        &self,
        Parameters(args): Parameters<GenerateCodeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({ "case_ids": args.case_ids });
        if let Some(v) = args.framework {
            body["framework"] = json!(v);
        }
        if let Some(v) = args.config_id {
            body["config_id"] = json!(v);
        }
        if let Some(v) = args.repo_id {
            body["repo_id"] = json!(v);
        }
        let path = format!("/api/v1/projects/{}/llm/generate-code", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "LLM chat propose: answer, clarify, or propose cases/requirements (POST …/llm/chat). Confirm with chat_confirm."
    )]
    async fn chat(
        &self,
        Parameters(args): Parameters<ChatArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({ "message": args.message });
        if let Some(v) = args.history {
            body["history"] = v;
        }
        if let Some(v) = args.config_id {
            body["config_id"] = json!(v);
        }
        let path = format!("/api/v1/projects/{}/llm/chat", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Confirm an LLM chat proposal (create proposed cases/requirements)")]
    async fn chat_confirm(
        &self,
        Parameters(args): Parameters<ChatConfirmArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/llm/chat/confirm", args.project_id);
        let value = self
            .client
            .post(&path, args.body)
            .await
            .map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "List requirements for a project (optional title search q)")]
    async fn list_requirements(
        &self,
        Parameters(args): Parameters<ListRequirementsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut path = format!("/api/v1/projects/{}/requirements", args.project_id);
        if let Some(q) = args.q {
            path.push_str(&format!("?q={}", urlencoding_lite(&q)));
        }
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Get a requirement by id")]
    async fn get_requirement(
        &self,
        Parameters(args): Parameters<GetRequirementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/requirements/{}",
            args.project_id, args.requirement_id
        );
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Create a requirement")]
    async fn create_requirement(
        &self,
        Parameters(args): Parameters<CreateRequirementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({ "title": args.title });
        if let Some(v) = args.description {
            body["description"] = json!(v);
        }
        if let Some(v) = args.acceptance_criteria {
            body["acceptance_criteria"] = json!(v);
        }
        if let Some(v) = args.folder_id {
            body["folder_id"] = json!(v);
        }
        if let Some(v) = args.source {
            body["source"] = json!(v);
        }
        if let Some(v) = args.external_key {
            body["external_key"] = json!(v);
        }
        if let Some(v) = args.url {
            body["url"] = json!(v);
        }
        let path = format!("/api/v1/projects/{}/requirements", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Link a case to a requirement")]
    async fn link_requirement(
        &self,
        Parameters(args): Parameters<LinkRequirementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/requirements:link", args.project_id);
        let value = self
            .client
            .post(
                &path,
                json!({
                    "case_id": args.case_id,
                    "requirement_id": args.requirement_id,
                }),
            )
            .await
            .map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "List campaigns for a project (display_id PREFIX-C-n never reused)")]
    async fn list_campaigns(
        &self,
        Parameters(args): Parameters<ProjectScoped>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/campaigns", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Get a campaign by UUID or display_id (PREFIX-C-n)")]
    async fn get_campaign(
        &self,
        Parameters(args): Parameters<GetCampaignArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/campaigns/{}",
            args.project_id, args.campaign_id
        );
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Create a campaign (gets never-reused PREFIX-C-n display_id)")]
    async fn create_campaign(
        &self,
        Parameters(args): Parameters<CreateCampaignArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({ "name": args.name });
        if let Some(v) = args.description {
            body["description"] = json!(v);
        }
        if let Some(v) = args.parent_id {
            body["parent_id"] = json!(v);
        }
        if let Some(v) = args.sort_order {
            body["sort_order"] = json!(v);
        }
        let path = format!("/api/v1/projects/{}/campaigns", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "List iterations for a project (optional campaign_id filter). Display ids are PREFIX-IT-n and never reused."
    )]
    async fn list_iterations(
        &self,
        Parameters(args): Parameters<ListIterationsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut path = format!("/api/v1/projects/{}/iterations", args.project_id);
        if let Some(cid) = args.campaign_id {
            path.push_str(&format!("?campaign_id={cid}"));
        }
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Get an iteration by UUID or display_id (PREFIX-IT-n)")]
    async fn get_iteration(
        &self,
        Parameters(args): Parameters<GetIterationArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/iterations/{}",
            args.project_id, args.iteration_id
        );
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Create an iteration under a campaign (gets never-reused PREFIX-IT-n)")]
    async fn create_iteration(
        &self,
        Parameters(args): Parameters<CreateIterationArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({
            "campaign_id": args.campaign_id,
            "name": args.name,
        });
        if let Some(v) = args.reference {
            body["reference"] = json!(v);
        }
        if let Some(v) = args.description {
            body["description"] = json!(v);
        }
        if let Some(v) = args.status {
            body["status"] = json!(v);
        }
        if let Some(v) = args.mode {
            body["mode"] = json!(v);
        }
        if let Some(v) = args.cadence {
            body["cadence"] = json!(v);
        }
        if let Some(v) = args.case_ids {
            body["case_ids"] = json!(v);
        }
        if let Some(v) = args.suite_ids {
            body["suite_ids"] = json!(v);
        }
        let path = format!("/api/v1/projects/{}/iterations", args.project_id);
        let value = self.client.post(&path, body).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "List automation/source repositories linked to a project")]
    async fn list_repos(
        &self,
        Parameters(args): Parameters<ProjectScoped>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/automation-repos", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Get an automation/source repository by id")]
    async fn get_repo(
        &self,
        Parameters(args): Parameters<RepoIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/automation-repos/{}", args.repo_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(
        description = "Create an automation/source repository and optionally link project_ids (multi-repo per project)."
    )]
    async fn create_repo(
        &self,
        Parameters(args): Parameters<CreateAutomationRepoArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut body = json!({
            "name": args.name,
            "provider": args.provider,
            "remote_url": args.remote_url,
        });
        if let Some(v) = args.branch {
            body["branch"] = json!(v);
        }
        if let Some(v) = args.subpath {
            body["subpath"] = json!(v);
        }
        if let Some(v) = args.token {
            body["token"] = json!(v);
        }
        if let Some(v) = args.actions_token {
            body["actions_token"] = json!(v);
        }
        if let Some(v) = args.ci_workflow_file {
            body["ci_workflow_file"] = json!(v);
        }
        if let Some(v) = args.ci_workflow_ref {
            body["ci_workflow_ref"] = json!(v);
        }
        if let Some(v) = args.poll_interval_minutes {
            body["poll_interval_minutes"] = json!(v);
        }
        if let Some(v) = args.project_ids {
            body["project_ids"] = json!(v);
        }
        let value = self
            .client
            .post("/api/v1/automation-repos", body)
            .await
            .map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Project reports dashboard (pass/fail overview)")]
    async fn report_dashboard(
        &self,
        Parameters(args): Parameters<ProjectScoped>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/reports/dashboard", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Run summary report (by run UUID, slug, or display id)")]
    async fn report_run_summary(
        &self,
        Parameters(args): Parameters<ReportRunArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/reports/runs/{}/summary",
            args.project_id, args.run_id
        );
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Full run report detail")]
    async fn report_run_detail(
        &self,
        Parameters(args): Parameters<ReportRunArgs>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!(
            "/api/v1/projects/{}/reports/runs/{}",
            args.project_id, args.run_id
        );
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Trend report across recent runs")]
    async fn report_trends(
        &self,
        Parameters(args): Parameters<ReportTrendsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut path = format!("/api/v1/projects/{}/reports/trends", args.project_id);
        if let Some(v) = args.limit {
            path.push_str(&format!("?limit={v}"));
        }
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Requirement/case coverage report")]
    async fn report_coverage(
        &self,
        Parameters(args): Parameters<ProjectScoped>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/reports/coverage", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
        Self::ok_json(value)
    }

    #[tool(description = "Flaky-test report for a project")]
    async fn report_flaky(
        &self,
        Parameters(args): Parameters<ProjectScoped>,
    ) -> Result<CallToolResult, McpError> {
        let path = format!("/api/v1/projects/{}/reports/flaky", args.project_id);
        let value = self.client.get(&path).await.map_err(Self::map_err)?;
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
