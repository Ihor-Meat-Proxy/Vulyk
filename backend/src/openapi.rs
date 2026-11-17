use serde_json::json;
use utoipa::{
    openapi::{
        content::ContentBuilder,
        response::ResponseBuilder,
        security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
        server::ServerBuilder,
        Ref, RefOr,
    },
    Modify, OpenApi,
};

use crate::auth::{ProjectRole, RoleMatrixRow};
use crate::error::{LegacyErrorDetail, ProblemDetails};
use crate::integrations::{DefectDraft, RemoteIssue, RemoteRequirement, RequirementLink};
use crate::models::{
    AbandonRunRequest, AddFieldToProject, ApiTokenPublic, AskTestsRequest, AskTestsResponse,
    AttachmentMeta, AuthResponse, AuthStatus, AutoCreatedCaseSummary, AutomatedSuite,
    AutomatedSuitesQuery, AutomationRepoBranch, AutomationRepoBranchesResponse,
    AutomationRepoIndexResult, AutomationRepoPublic, AutomationRepoTestResult, BackupDestination,
    BackupFieldMapping, BackupPolicy, BackupRun, BackupTestResult, BootstrapRequest,
    BulkCasesRequest, BulkCasesResponse, BulkIngestResults, BulkResultItem, BulkResultsRequest,
    BulkResultsResponse, Campaign, CaseExecutionHistoryItem, CaseExecutionHistoryPage,
    CaseListColumnPref, CaseListColumnsConfig, CaseRequirementLink, CaseResultsPage,
    CaseResultsSummary, CaseVersion, CaseVersionSummary, ChangePasswordRequest, ChatConfirmRequest,
    ChatConfirmResponse, ChatCreationEventPublic, ChatMessage, ChatPlacement, ChatProposal,
    ChatProposeRequest, ChatProposeResponse, ChatResponseKind, CiConfig, CiConfigPublic,
    CiConfigTestResult, CiDispatch, CiRunResultsResponse, CloseCiRunRequest, CompleteSetupRequest,
    ConfigBundle, ConfigExportRequest, ConfigImportPreview, ConfigImportRequest, ContentArchive,
    ContentArchiveManifest, ContextFileRef, CoverageDetailRow, CoverageLinkedCase, CreateApiToken,
    CreateApiTokenResponse, CreateAttachment, CreateAutomationRepo, CreateBackupDestination,
    CreateBackupPolicy, CreateCampaign, CreateCase, CreateCiConfig, CreateCustomField,
    CreateCustomFieldDefinition, CreateDefectRequest, CreateIntegration, CreateIteration,
    CreateLlmConfig, CreateOptionList, CreateOptionListItem, CreateOutboundWebhook,
    CreateOutboundWebhookResponse, CreatePlan, CreateProject, CreateProjectCiToken,
    CreateProjectCiTokenResponse, CreateRequirement, CreateRequirementFolder, CreateResult,
    CreateRun, CreateStep, CreateSuite, CreateTag, CreateUserRequest, CustomField,
    CustomFieldDefinition, CustomFieldDefinitionDetail, CustomFieldProjectOverride,
    CustomFieldValue, Defect, DeleteSuiteQuery, DispatchCiRequest, ExecutionsTreeNode, ExportQuery,
    FieldMapping, FlakyCase, FlakyQuery, FlakyReport, FlakyTestSummary, GenerateCasesRequest,
    GenerateCasesResponse, GenerateCodeRequest, GenerateCodeResponse, GeneratedCaseDraft,
    ImportCaseDraft, ImportPreview, ImportPreviewRequest, ImportReport, ImportRunRequest,
    ImportStepDraft, ImporterInfo, IngestRequest, IngestResponse, IngestResultItem,
    IntegrationPublic, IntegrationSetting, Iteration, IterationDashboard, IterationItem,
    IterationItemView, IterationStatusCounts, Job, LinkCaseRequirement, ListBranchesBody,
    LlmConfigPublic, LoginRequest, ManualBackupRequest, Membership, MembershipView,
    OpenCiRunRequest, OpenCiRunResponse, OptionList, OptionListDetail, OptionListItem,
    OutboundWebhookDeliveryPublic, OutboundWebhookEndpointPublic, Page, PassFailTrends,
    PassRatePoint, PipelineIngestRequest, PipelineIngestResponse, Project, ProjectCiTokenPublic,
    ProjectDashboard, ProposedCaseDraft, ProposedLinkDraft, ProposedRequirementDraft,
    RegisterRequest, ReorderOptionListItems, Requirement, RequirementCoverageDetail,
    RequirementCoverageReport, RequirementCoverageRow, RequirementFolder, RestoreConflict,
    RestoreCounts, RestorePreview, RestoreRequest, ResultStep, RunCaseResultExpand,
    RunCaseResultRow, RunCaseResultsPage, RunInCiRequest, RunInCiResponse, RunItem,
    RunItemExecution, RunItemView, RunProgress, RunReportDetail, RunResultAttachment,
    RunResultCommentEntry, RunResultDefect, RunResultGroup, RunResultGroups, RunResultStepRow,
    RunSummaryReport, ScopeChangeWarning, SetCustomFieldValue, SetProjectDefaultRepo,
    SetRepoProjects, SetupStatus, SlowTestSummary, SsoProviderPublic, StartCiRunRequest,
    StatusCounts, Suite, SuiteResultRow, Tag, TestAutomationRepoBody, TestCase, TestCaseDetail,
    TestCaseListItem, TestPlan, TestPlanDetail, TestResult, TestResultDetail,
    TestResultHistoryItem, TestRun, TestRunDetail, TestStep, TrendPoint, TrendsQuery,
    UpdateAutomationRepo, UpdateBackupDestination, UpdateBackupPolicy, UpdateCampaign, UpdateCase,
    UpdateCiConfig, UpdateCustomField, UpdateCustomFieldDefinition, UpdateIntegration,
    UpdateIteration, UpdateLlmConfig, UpdateMeRequest, UpdateMembership, UpdateOptionList,
    UpdateOptionListItem, UpdateOutboundWebhook, UpdatePlan, UpdatePreferences, UpdateProject,
    UpdateRequirement, UpdateRun, UpdateRunItem, UpdateSuite, UpdateTag, UpdateUserRequest,
    UpsertAutomationRepo, UpsertIntegrationSetting, UpsertMembership, UpsertProjectFieldOverride,
    UserPreferences, UserPublic, WebhookAck,
};
use crate::routes::custom_fields::UpdateFieldResponse;
use crate::routes::health::{HealthResponse, ReadyResponse};
use crate::routes::instance_settings::{RoleMatrixResponse, UpdateInstanceSettings};
use crate::routes::llm::ProviderKindInfo;
use crate::routes::sample_data::{ReseedSampleDataRequest, SampleDataStatus, SampleProjectInfo};
use crate::routes::updates::{
    ApplyRollbackRequest, ApplyUpdateRequest, ApplyUpdateResponse, RollbackPointInfo,
    UpdateJobList, UpdateJobPublic, UpdatePreferencesRequest, UpdatesCapability,
    UpdatesPreferences, UpdatesStatus, VersionBackupInfo, VersionBackupList,
    VersionBackupRetention,
};
use crate::sample_seed::SampleDataSummary;
use crate::settings::{InstanceSettings, RegistrationMode};
use crate::sso::{SsoProviderAdmin, UpdateSsoProviderRequest};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Vulyk API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Open-source test management system (beehive). Projects (optional key, case_key_prefix + never-reused PREFIX-N case numbers), nested folder trees with recursive counts, plans/runs, automation executions (campaigns/iterations/pipeline ingest), ALM/importers/reports, LLM generate/chat (propose/create), MCP, locales en/uk/fr/es/de, deploy.",
        license(name = "AGPL-3.0")
    ),
    paths(
        crate::routes::health::health,
        crate::routes::health::ready,
        crate::routes::api_docs::docs_html,
        crate::routes::api_docs::scalar_js,
        crate::routes::api_docs::openapi_json,
        crate::routes::auth::auth_status,
        crate::routes::auth::bootstrap,
        crate::routes::auth::register,
        crate::routes::auth::login,
        crate::routes::auth::logout,
        crate::routes::auth::me,
        crate::routes::auth::update_me,
        crate::routes::auth::change_password,
        crate::routes::auth::sso_providers,
        crate::routes::auth::sso_start,
        crate::routes::auth::sso_callback,
        crate::routes::setup::setup_status,
        crate::routes::setup::setup_complete,
        crate::routes::sso_admin::list_providers,
        crate::routes::sso_admin::update_provider,
        crate::routes::sso_admin::patch_provider,
        crate::routes::instance_settings::get_settings,
        crate::routes::instance_settings::put_settings,
        crate::routes::instance_settings::get_role_matrix,
        crate::routes::updates::get_updates,
        crate::routes::updates::check_updates,
        crate::routes::updates::apply_update,
        crate::routes::updates::apply_rollback,
        crate::routes::updates::list_jobs,
        crate::routes::updates::get_job,
        crate::routes::updates::list_version_backups,
        crate::routes::updates::download_version_backup,
        crate::routes::updates::get_prefs,
        crate::routes::updates::put_prefs,
        crate::routes::users::list_users,
        crate::routes::users::create_user,
        crate::routes::users::get_user,
        crate::routes::users::update_user,
        crate::routes::users::delete_user,
        crate::routes::memberships::list_memberships,
        crate::routes::memberships::upsert_membership,
        crate::routes::memberships::update_membership,
        crate::routes::memberships::delete_membership,
        crate::routes::admin::list_integrations,
        crate::routes::admin::create_integration,
        crate::routes::admin::delete_integration,
        crate::routes::sample_data::get_sample_data,
        crate::routes::sample_data::delete_sample_data,
        crate::routes::sample_data::reseed_sample_data,
        crate::routes::projects::list_projects,
        crate::routes::projects::create_project,
        crate::routes::projects::get_project,
        crate::routes::projects::update_project,
        crate::routes::projects::delete_project,
        crate::routes::suites::list_suites,
        crate::routes::suites::create_suite,
        crate::routes::suites::get_suite,
        crate::routes::suites::update_suite,
        crate::routes::suites::delete_suite,
        crate::routes::cases::list_cases,
        crate::routes::project_preferences::get_case_list_columns,
        crate::routes::project_preferences::put_case_list_columns,
        crate::routes::project_preferences::delete_case_list_columns,
        crate::routes::cases::create_case,
        crate::routes::cases::get_case,
        crate::routes::cases::get_case_by_key,
        crate::routes::cases::update_case,
        crate::routes::cases::delete_case,
        crate::routes::attachments::create_attachment,
        crate::routes::attachments::delete_attachment,
        crate::routes::attachments::download_attachment,
        crate::routes::attachments::list_attachments,
        crate::routes::cases::bulk_cases,
        crate::routes::cases::export_case_pdf_project,
        crate::routes::cases::export_case_pdf_by_id_or_key,
        crate::routes::cases::export_cases_bulk_handler,
        crate::routes::cases::get_version,
        crate::routes::cases::list_versions,
        crate::routes::cases::list_case_results_project,
        crate::routes::cases::list_case_results_by_id,
        crate::routes::cases::search_cases,
        crate::routes::cases::set_custom_fields,
        crate::routes::custom_fields::admin_list_fields,
        crate::routes::custom_fields::admin_create_field,
        crate::routes::custom_fields::admin_get_field,
        crate::routes::custom_fields::admin_update_field,
        crate::routes::custom_fields::admin_delete_field,
        crate::routes::custom_fields::create_field_compat,
        crate::routes::custom_fields::delete_field_compat,
        crate::routes::custom_fields::get_field,
        crate::routes::custom_fields::list_fields,
        crate::routes::custom_fields::list_available,
        crate::routes::custom_fields::add_field_to_project,
        crate::routes::custom_fields::upsert_override,
        crate::routes::custom_fields::update_field_compat,
        crate::routes::option_lists::list_lists,
        crate::routes::option_lists::create_list,
        crate::routes::option_lists::get_list,
        crate::routes::option_lists::update_list,
        crate::routes::option_lists::delete_list,
        crate::routes::option_lists::create_item,
        crate::routes::option_lists::update_item,
        crate::routes::option_lists::delete_item,
        crate::routes::option_lists::reorder_items,
        crate::routes::config_export::export_config,
        crate::routes::config_export::import_config,
        crate::routes::config_export::import_preview,
        crate::routes::plans::create_plan,
        crate::routes::plans::delete_plan,
        crate::routes::plans::get_plan,
        crate::routes::plans::list_plans,
        crate::routes::plans::update_plan,
        crate::routes::campaigns::executions_tree,
        crate::routes::campaigns::list_campaigns,
        crate::routes::campaigns::create_campaign,
        crate::routes::campaigns::get_campaign,
        crate::routes::campaigns::update_campaign,
        crate::routes::campaigns::delete_campaign,
        crate::routes::campaigns::list_iterations,
        crate::routes::campaigns::create_iteration_top,
        crate::routes::campaigns::list_campaign_iterations,
        crate::routes::campaigns::create_iteration,
        crate::routes::campaigns::get_iteration,
        crate::routes::campaigns::update_iteration,
        crate::routes::campaigns::delete_iteration,
        crate::routes::campaigns::iteration_dashboard,
        crate::routes::campaigns::list_iteration_items,
        crate::routes::campaigns::list_suites,
        crate::routes::campaigns::list_suite_results,
        crate::routes::campaigns::delete_suite_run,
        crate::routes::campaigns::rerun_suite,
        crate::routes::campaigns::case_executions,
        crate::routes::campaigns::pipeline_ingest,
        crate::routes::runs::create_result,
        crate::routes::runs::delete_run,
        crate::routes::runs::get_run_item,
        crate::routes::runs::list_run_item_results,
        crate::routes::runs::list_run_results_history_endpoint,
        crate::routes::runs::list_run_items,
        crate::routes::runs::update_run_item,
        crate::routes::tags::create_tag,
        crate::routes::tags::delete_tag,
        crate::routes::tags::get_tag,
        crate::routes::tags::list_tags,
        crate::routes::tags::update_tag,
        crate::routes::runs::list_runs,
        crate::routes::runs::create_run,
        crate::routes::runs::get_run,
        crate::routes::runs::update_run,
        crate::routes::runs::trigger_run,
        crate::routes::runs::abandon_run,
        crate::routes::runs::list_results,
        crate::routes::runs::ingest_results,
        crate::routes::runs::get_run_by_id,
        crate::routes::runs::update_run_by_id,
        crate::routes::runs::delete_run_by_id,
        crate::routes::runs::list_results_by_id,
        crate::routes::reports::project_dashboard,
        crate::routes::reports::run_summary,
        crate::routes::reports::run_report_detail,
        crate::routes::reports::run_case_results,
        crate::routes::reports::run_case_expand,
        crate::routes::reports::run_groups,
        crate::routes::reports::trends,
        crate::routes::reports::coverage,
        crate::routes::reports::coverage_detail,
        crate::routes::reports::flaky,
        crate::routes::reports::export_report,
        crate::routes::ingest::bulk_results,
        crate::routes::ingest::ingest,
        crate::routes::jobs::get_job,
        crate::routes::ci::list_ci_configs,
        crate::routes::ci::create_ci_config,
        crate::routes::ci::get_ci_config,
        crate::routes::ci::update_ci_config,
        crate::routes::ci::delete_ci_config,
        crate::routes::ci::dispatch_ci,
        crate::routes::ci::test_ci_config,
        crate::routes::ci::run_in_ci,
        crate::routes::ci::list_dispatches,
        crate::routes::ci::get_dispatch,
        crate::routes::requirements::list_requirements,
        crate::routes::requirements::create_requirement,
        crate::routes::requirements::get_requirement,
        crate::routes::requirements::set_requirement_custom_fields,
        crate::routes::requirements::update_requirement,
        crate::routes::requirements::delete_requirement,
        crate::routes::requirements::link_case,
        crate::routes::requirements::unlink_case,
        crate::routes::requirements::list_case_requirements,
        crate::routes::integrations::list_integrations,
        crate::routes::integrations::create_integration,
        crate::routes::integrations::get_integration,
        crate::routes::integrations::update_integration,
        crate::routes::integrations::delete_integration,
        crate::routes::integrations::search_remote_requirements,
        crate::routes::integrations::import_remote_requirements,
        crate::routes::integrations::list_defects,
        crate::routes::integrations::create_defect,
        crate::routes::imports::list_importers,
        crate::routes::imports::preview_import,
        crate::routes::imports::run_import,
        crate::routes::llm::list_provider_kinds,
        crate::routes::llm::list_configs,
        crate::routes::llm::create_config,
        crate::routes::llm::get_config,
        crate::routes::llm::update_config,
        crate::routes::llm::delete_config,
        crate::routes::llm::generate_cases,
        crate::routes::llm::generate_code,
        crate::routes::llm::ask_tests,
        crate::routes::chat_propose::list_chat_tools,
        crate::routes::chat_propose::chat_propose,
        crate::routes::chat_propose::chat_confirm,
        crate::routes::requirements::list_requirement_folders,
        crate::routes::requirements::create_requirement_folder,
        crate::routes::automation_repos::list_repos,
        crate::routes::automation_repos::create_repo,
        crate::routes::automation_repos::test_repo_preview,
        crate::routes::automation_repos::get_repo_by_id,
        crate::routes::automation_repos::update_repo,
        crate::routes::automation_repos::delete_repo_by_id,
        crate::routes::automation_repos::set_repo_projects,
        crate::routes::automation_repos::test_repo_by_id,
        crate::routes::automation_repos::reindex_repo_by_id,
        crate::routes::automation_repos::list_repo_branches,
        crate::routes::automation_repos::list_branches_preview,
        crate::routes::automation_repos::list_project_repos,
        crate::routes::automation_repos::set_project_default_repo,
        crate::routes::automation_repos::get_repo,
        crate::routes::automation_repos::upsert_repo,
        crate::routes::automation_repos::delete_repo,
        crate::routes::automation_repos::test_repo,
        crate::routes::automation_repos::reindex_repo,
        crate::routes::ci_report::list_ci_tokens,
        crate::routes::ci_report::create_ci_token,
        crate::routes::ci_report::revoke_ci_token,
        crate::routes::ci_report::open_ci_run,
        crate::routes::ci_report::get_ci_run,
        crate::routes::ci_report::start_ci_run,
        crate::routes::ci_report::post_ci_results,
        crate::routes::ci_report::ingest_ci_run,
        crate::routes::ci_report::close_ci_run,
        crate::routes::outbound_webhooks::list_endpoints,
        crate::routes::outbound_webhooks::create_endpoint,
        crate::routes::outbound_webhooks::get_endpoint,
        crate::routes::outbound_webhooks::update_endpoint,
        crate::routes::outbound_webhooks::delete_endpoint,
        crate::routes::outbound_webhooks::list_deliveries,
        crate::routes::outbound_webhooks::redeliver_delivery,
        crate::routes::outbound_webhooks::test_endpoint,
        crate::routes::tokens::list_tokens,
        crate::routes::tokens::create_token,
        crate::routes::tokens::revoke_token,
        crate::routes::backups::list_destinations,
        crate::routes::backups::create_destination,
        crate::routes::backups::get_destination,
        crate::routes::backups::update_destination,
        crate::routes::backups::delete_destination,
        crate::routes::backups::test_destination,
        crate::routes::backups::list_policies,
        crate::routes::backups::create_policy,
        crate::routes::backups::update_policy,
        crate::routes::backups::delete_policy,
        crate::routes::backups::run_manual,
        crate::routes::backups::list_runs,
        crate::routes::backups::get_run,
        crate::routes::backups::download_run,
        crate::routes::backups::restore,
        crate::routes::backups::restore_upload,
        crate::routes::webhooks::github_webhook,
        crate::routes::webhooks::github_webhook_for_config,
        crate::routes::webhooks::forgejo_webhook,
        crate::routes::webhooks::forgejo_webhook_for_config,
        crate::routes::webhooks::gitea_webhook,
        crate::routes::webhooks::gitea_webhook_for_config,
        crate::routes::webhooks::gitlab_webhook,
        crate::routes::webhooks::gitlab_webhook_for_config,
        crate::routes::webhooks::automation_repo_webhook,
    ),
    components(schemas(
        ProblemDetails,
        LegacyErrorDetail,
        HealthResponse,
        ReadyResponse,
        Project,
        CreateProject,
        UpdateProject,
        crate::models::CiUnmatchedPolicy,
        Suite,
        CreateSuite,
        UpdateSuite,
        DeleteSuiteQuery,
        TestCase,
        TestStep,
        TestCaseDetail,
        CreateCase,
        CreateStep,
        UpdateCase,
        TestRun,
        CreateRun,
        UpdateRun,
        AbandonRunRequest,
        TestResult,
        BulkIngestResults,
        IngestResultItem,
        BulkResultItem,
        BulkResultsRequest,
        BulkResultsResponse,
        AutoCreatedCaseSummary,
        IngestRequest,
        IngestResponse,
        Job,
        CiConfig,
        CiConfigPublic,
        CiConfigTestResult,
        CreateCiConfig,
        UpdateCiConfig,
        CiDispatch,
        DispatchCiRequest,
        RunInCiRequest,
        RunInCiResponse,
        WebhookAck,
        Requirement,
        CreateRequirement,
        UpdateRequirement,
        LinkCaseRequirement,
        CaseRequirementLink,
        IntegrationPublic,
        CreateIntegration,
        UpdateIntegration,
        Defect,
        CreateDefectRequest,
        DefectDraft,
        RemoteRequirement,
        RemoteIssue,
        RequirementLink,
        ImporterInfo,
        FieldMapping,
        ImportCaseDraft,
        ImportStepDraft,
        ImportPreview,
        ImportPreviewRequest,
        ImportRunRequest,
        ImportReport,
        Page<TestCase>,
        Page<TestCaseListItem>,
        TestCaseListItem,
        CaseListColumnPref,
        CaseListColumnsConfig,
        Tag,
        CreateTag,
        UpdateTag,
        CustomField,
        CustomFieldDefinition,
        CustomFieldDefinitionDetail,
        CustomFieldProjectOverride,
        CreateCustomField,
        CreateCustomFieldDefinition,
        UpdateCustomField,
        UpdateCustomFieldDefinition,
        UpdateFieldResponse,
        ScopeChangeWarning,
        AddFieldToProject,
        UpsertProjectFieldOverride,
        CustomFieldValue,
        SetCustomFieldValue,
        OptionList,
        OptionListItem,
        OptionListDetail,
        CreateOptionList,
        UpdateOptionList,
        CreateOptionListItem,
        UpdateOptionListItem,
        ReorderOptionListItems,
        ConfigBundle,
        ConfigExportRequest,
        ConfigImportRequest,
        ConfigImportPreview,
        CaseVersion,
        CaseVersionSummary,
        BulkCasesRequest,
        BulkCasesResponse,
        crate::services::case_export::BulkExportRequest,
        crate::services::case_export::ExportQuery,
        TestPlan,
        TestPlanDetail,
        CreatePlan,
        UpdatePlan,
        Campaign,
        CreateCampaign,
        UpdateCampaign,
        Iteration,
        CreateIteration,
        UpdateIteration,
        IterationItem,
        IterationItemView,
        IterationStatusCounts,
        PassRatePoint,
        FlakyTestSummary,
        SlowTestSummary,
        IterationDashboard,
        ExecutionsTreeNode,
        AutomatedSuite,
        AutomatedSuitesQuery,
        SuiteResultRow,
        CaseExecutionHistoryItem,
        CaseExecutionHistoryPage,
        PipelineIngestRequest,
        PipelineIngestResponse,
        Page<AutomatedSuite>,
        Page<SuiteResultRow>,
        TestRunDetail,
        RunProgress,
        RunItem,
        RunItemView,
        UpdateRunItem,
        RunItemExecution,
        TestResultDetail,
        TestResultHistoryItem,
        CaseResultsSummary,
        CaseResultsPage,
        ResultStep,
        CreateResult,
        Page<TestRun>,
        AttachmentMeta,
        CreateAttachment,
        LlmConfigPublic,
        CreateLlmConfig,
        UpdateLlmConfig,
        ProviderKindInfo,
        GenerateCasesRequest,
        GenerateCasesResponse,
        GeneratedCaseDraft,
        GenerateCodeRequest,
        GenerateCodeResponse,
        ContextFileRef,
        AutomationRepoPublic,
        CreateAutomationRepo,
        UpdateAutomationRepo,
        UpsertAutomationRepo,
        SetRepoProjects,
        SetProjectDefaultRepo,
        TestAutomationRepoBody,
        ListBranchesBody,
        AutomationRepoBranch,
        AutomationRepoBranchesResponse,
        AutomationRepoTestResult,
        AutomationRepoIndexResult,
        ProjectCiTokenPublic,
        CreateProjectCiToken,
        CreateProjectCiTokenResponse,
        OpenCiRunRequest,
        StartCiRunRequest,
        OpenCiRunResponse,
        CloseCiRunRequest,
        CiRunResultsResponse,
        OutboundWebhookEndpointPublic,
        OutboundWebhookDeliveryPublic,
        CreateOutboundWebhook,
        CreateOutboundWebhookResponse,
        UpdateOutboundWebhook,
        AskTestsRequest,
        AskTestsResponse,
        ChatMessage,
        ChatProposeRequest,
        ChatProposeResponse,
        ChatResponseKind,
        ChatPlacement,
        ChatProposal,
        ProposedCaseDraft,
        ProposedRequirementDraft,
        ProposedLinkDraft,
        ChatConfirmRequest,
        ChatConfirmResponse,
        ChatCreationEventPublic,
        RequirementFolder,
        CreateRequirementFolder,
        ApiTokenPublic,
        CreateApiToken,
        CreateApiTokenResponse,
        UserPublic,
        UserPreferences,
        UpdatePreferences,
        BootstrapRequest,
        LoginRequest,
        RegisterRequest,
        ChangePasswordRequest,
        UpdateMeRequest,
        CompleteSetupRequest,
        SetupStatus,
        SsoProviderPublic,
        SsoProviderAdmin,
        UpdateSsoProviderRequest,
        InstanceSettings,
        UpdateInstanceSettings,
        UpdatesStatus,
        UpdatesCapability,
        ApplyUpdateRequest,
        ApplyRollbackRequest,
        ApplyUpdateResponse,
        UpdateJobPublic,
        UpdateJobList,
        UpdatesPreferences,
        UpdatePreferencesRequest,
        VersionBackupInfo,
        VersionBackupList,
        VersionBackupRetention,
        RollbackPointInfo,
        SampleDataStatus,
        SampleProjectInfo,
        SampleDataSummary,
        ReseedSampleDataRequest,
        RegistrationMode,
        RoleMatrixRow,
        RoleMatrixResponse,
        AuthResponse,
        AuthStatus,
        crate::auth::PasswordPolicyPublic,
        CreateUserRequest,
        UpdateUserRequest,
        ProjectRole,
        Membership,
        MembershipView,
        UpsertMembership,
        UpdateMembership,
        IntegrationSetting,
        UpsertIntegrationSetting,
        StatusCounts,
        RunSummaryReport,
        RunReportDetail,
        RunCaseResultRow,
        RunCaseResultsPage,
        RunResultDefect,
        RunResultAttachment,
        RunResultStepRow,
        RunResultCommentEntry,
        RunCaseResultExpand,
        RunResultGroup,
        RunResultGroups,
        CoverageLinkedCase,
        CoverageDetailRow,
        RequirementCoverageDetail,
        TrendPoint,
        PassFailTrends,
        RequirementCoverageRow,
        RequirementCoverageReport,
        FlakyCase,
        FlakyReport,
        ProjectDashboard,
        TrendsQuery,
        FlakyQuery,
        ExportQuery,
        BackupDestination,
        CreateBackupDestination,
        UpdateBackupDestination,
        BackupPolicy,
        CreateBackupPolicy,
        UpdateBackupPolicy,
        BackupRun,
        ManualBackupRequest,
        BackupTestResult,
        RestoreRequest,
        RestorePreview,
        RestoreCounts,
        RestoreConflict,
        BackupFieldMapping,
        ContentArchive,
        ContentArchiveManifest,
    )),
    modifiers(&SecurityAddon, &OpenApiQualityAddon),
    tags(
        (name = "system", description = "Health and readiness"),
        (name = "auth", description = "Local login, sessions, JWT"),
        (name = "users", description = "User administration"),
        (name = "memberships", description = "Per-project RBAC"),
        (name = "admin", description = "Admin settings placeholders"),
        (name = "backups", description = "Application-level content backup and restore"),
        (name = "projects", description = "Projects"),
        (name = "suites", description = "Suites / sections"),
        (name = "cases", description = "Test cases and steps"),
        (name = "preferences", description = "Per-user per-project preferences (case list columns)"),
        (name = "runs", description = "Test runs and results"),
        (name = "reports", description = "Dashboards, trends, coverage, flaky detection, export"),
        (name = "ingest", description = "Bulk / JUnit / Vulyk JSON ingest"),
        (name = "jobs", description = "Background jobs"),
        (name = "ci", description = "CI configs and dispatches"),
        (name = "ci-report", description = "Project CI tokens and CI run lifecycle reporting"),
        (name = "webhooks", description = "GitHub / GitLab status webhooks"),
        (name = "requirements", description = "Requirements and traceability"),
        (name = "integrations", description = "ALM integrations and defects"),
        (name = "imports", description = "Case importers"),
        (name = "tags", description = "Case tags"),
        (name = "custom_fields", description = "Custom field definitions and project fields"),
        (name = "option_lists", description = "Reusable Admin option lists"),
        (name = "config", description = "Configuration export and import"),
        (name = "plans", description = "Test plans"),
        (name = "attachments", description = "Attachments for cases/results/runs"),
        (name = "llm", description = "LLM providers and generation"),
        (name = "automation-repo", description = "Test automation Git repositories (multi-project)"),
        (name = "tokens", description = "API tokens for MCP / automation"),
        (name = "outbound-webhooks", description = "Signed outbound webhooks for case/run/result events"),
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Same-origin relative server so Scalar Try it hits this instance (air-gapped; no proxy).
        openapi.servers = Some(vec![ServerBuilder::new()
            .url("/")
            .description(Some(
                "This Vulyk instance (same origin). Scalar Test Request calls this server; CSP connect-src is self only.",
            ))
            .build()]);

        let auth_help = concat!(
            "\n\n## Auth in /api/docs (Scalar)\n\n",
            "1. **Bearer** — open **Auth**, select `bearer_auth`, paste a JWT from `POST /api/v1/auth/login` ",
            "(`token` field) or a personal API token from Settings → Tokens. Scalar sends `Authorization: Bearer …`.\n",
            "2. **Session cookie** — if you are already signed in on this origin, the browser sends the HttpOnly ",
            "`vulyk_session` cookie with Test Request automatically (no paste). Cookie auth is not editable in Scalar; ",
            "use Bearer when calling from another tool.\n",
            "3. Search uses the sidebar search box (Ctrl/⌘+K). Test Request runs against the same-origin server (`/`).",
            "\n\n## Errors (RFC 9457)\n\n",
            "Error responses use `Content-Type: application/problem+json` with fields ",
            "`type`, `title`, `status`, `detail`, and extension `code` (stable machine code). ",
            "A nested `error: { code, message }` object is dual-written for older clients. ",
            "See shared response components `ProblemBadRequest`, `ProblemUnauthorized`, ",
            "`ProblemForbidden`, and `ProblemNotFound`.",
        );
        match openapi.info.description.as_mut() {
            Some(desc) => desc.push_str(auth_help),
            None => openapi.info.description = Some(auth_help.trim_start().into()),
        }

        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT or API token")
                        .description(Some(concat!(
                            "Paste a session JWT (`POST /api/v1/auth/login` → `token`) or a project-scoped personal API token ",
                            "(Settings → Tokens / `POST /api/v1/tokens` with `project_id` + `permissions`). ",
                            "Alternatively, when browsing /api/docs on this origin while logged in, the HttpOnly ",
                            "`vulyk_session` cookie is sent automatically with Test Request.",
                        )))
                        .build(),
                ),
            );
        }
    }
}

/// Fill operation summaries and shared RFC 9457 problem response components/examples.
struct OpenApiQualityAddon;

fn humanize_operation_id(id: &str) -> String {
    let mut parts = id.split('_').filter(|p| !p.is_empty());
    let Some(first) = parts.next() else {
        return id.to_string();
    };
    let mut out = String::new();
    let mut chars = first.chars();
    if let Some(c) = chars.next() {
        out.extend(c.to_uppercase());
        out.push_str(chars.as_str());
    }
    for part in parts {
        out.push(' ');
        out.push_str(part);
    }
    out
}

fn problem_example(status: u16, code: &str, title: &str, detail: &str) -> serde_json::Value {
    json!({
        "type": format!("urn:vulyk:api:problem:{code}"),
        "title": title,
        "status": status,
        "detail": detail,
        "code": code,
        "error": { "code": code, "message": detail }
    })
}

fn problem_response_component(
    description: &str,
    status: u16,
    code: &str,
    title: &str,
    detail: &str,
) -> utoipa::openapi::Response {
    ResponseBuilder::new()
        .description(description)
        .content(
            "application/problem+json",
            ContentBuilder::new()
                .schema(Some(RefOr::Ref(Ref::from_schema_name("ProblemDetails"))))
                .example(Some(problem_example(status, code, title, detail)))
                .build(),
        )
        .build()
}

fn ensure_operation_summary(op: &mut utoipa::openapi::path::Operation) {
    let needs_summary = op
        .summary
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if !needs_summary {
        return;
    }
    if let Some(id) = op.operation_id.as_deref() {
        op.summary = Some(humanize_operation_id(id));
        return;
    }
    // Fall back to the first success response description when present.
    for code in ["200", "201", "204"] {
        if let Some(RefOr::T(resp)) = op.responses.responses.get(code) {
            if !resp.description.trim().is_empty() {
                op.summary = Some(resp.description.trim().to_string());
                return;
            }
        }
    }
}

fn shared_problem_ref(status: &str) -> Option<&'static str> {
    match status {
        "400" => Some("ProblemBadRequest"),
        "401" => Some("ProblemUnauthorized"),
        "403" => Some("ProblemForbidden"),
        "404" => Some("ProblemNotFound"),
        "409" => Some("ProblemConflict"),
        "410" => Some("ProblemGone"),
        "429" => Some("ProblemTooManyRequests"),
        "500" | "503" => Some("ProblemInternal"),
        _ => None,
    }
}

fn default_response_description(status: &str) -> &'static str {
    match status {
        "200" => "OK",
        "201" => "Created",
        "204" => "No Content",
        "400" => "Bad Request",
        "401" => "Unauthorized",
        "403" => "Forbidden",
        "404" => "Not Found",
        "409" => "Conflict",
        "410" => "Gone",
        "429" => "Too Many Requests",
        "500" => "Internal Server Error",
        "503" => "Service Unavailable",
        _ => "Response",
    }
}

fn enrich_error_responses(op: &mut utoipa::openapi::path::Operation) {
    let statuses: Vec<String> = op.responses.responses.keys().cloned().collect();
    for status in statuses {
        let Some(entry) = op.responses.responses.get_mut(&status) else {
            continue;
        };
        // OAS requires Response.description; utoipa omits empty strings from JSON.
        if let RefOr::T(resp) = entry {
            if resp.description.trim().is_empty() {
                resp.description = default_response_description(&status).to_string();
            }
        }
        let Some(component) = shared_problem_ref(&status) else {
            continue;
        };
        match entry {
            RefOr::Ref(_) => {}
            RefOr::T(resp) if resp.content.is_empty() => {
                *entry = RefOr::Ref(Ref::from_response_name(component));
            }
            RefOr::T(resp) => {
                // Documented body without problem media type — ensure problem+json is present.
                if !resp.content.contains_key("application/problem+json") {
                    resp.content.insert(
                        "application/problem+json".into(),
                        ContentBuilder::new()
                            .schema(Some(RefOr::Ref(Ref::from_schema_name("ProblemDetails"))))
                            .build()
                            .into(),
                    );
                }
            }
        }
    }
}

impl Modify for OpenApiQualityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            let shared = [
                (
                    "ProblemBadRequest",
                    problem_response_component(
                        "Bad request (RFC 9457 problem+json)",
                        400,
                        "bad_request",
                        "Bad Request",
                        "bad request: invalid input",
                    ),
                ),
                (
                    "ProblemUnauthorized",
                    problem_response_component(
                        "Unauthorized (RFC 9457 problem+json)",
                        401,
                        "unauthorized",
                        "Unauthorized",
                        "unauthorized: authentication required",
                    ),
                ),
                (
                    "ProblemForbidden",
                    problem_response_component(
                        "Forbidden (RFC 9457 problem+json)",
                        403,
                        "forbidden",
                        "Forbidden",
                        "forbidden: insufficient permissions",
                    ),
                ),
                (
                    "ProblemNotFound",
                    problem_response_component(
                        "Not found (RFC 9457 problem+json)",
                        404,
                        "not_found",
                        "Not Found",
                        "not found: resource",
                    ),
                ),
                (
                    "ProblemConflict",
                    problem_response_component(
                        "Conflict (RFC 9457 problem+json)",
                        409,
                        "conflict",
                        "Conflict",
                        "conflict: resource already exists",
                    ),
                ),
                (
                    "ProblemGone",
                    problem_response_component(
                        "Gone (RFC 9457 problem+json)",
                        410,
                        "case_deleted",
                        "Gone",
                        "gone: case key retired",
                    ),
                ),
                (
                    "ProblemTooManyRequests",
                    problem_response_component(
                        "Too many requests (RFC 9457 problem+json)",
                        429,
                        "too_many_requests",
                        "Too Many Requests",
                        "too many requests: rate limited",
                    ),
                ),
                (
                    "ProblemInternal",
                    problem_response_component(
                        "Internal server error (RFC 9457 problem+json)",
                        500,
                        "internal",
                        "Internal Server Error",
                        "internal: unexpected error",
                    ),
                ),
            ];
            for (name, response) in shared {
                components
                    .responses
                    .insert(name.to_string(), RefOr::T(response));
            }
        }

        for path_item in openapi.paths.paths.values_mut() {
            for op in [
                path_item.get.as_mut(),
                path_item.put.as_mut(),
                path_item.post.as_mut(),
                path_item.delete.as_mut(),
                path_item.options.as_mut(),
                path_item.head.as_mut(),
                path_item.patch.as_mut(),
                path_item.trace.as_mut(),
                path_item.query.as_mut(),
            ]
            .into_iter()
            .flatten()
            .chain(path_item.additional_operations.values_mut())
            {
                ensure_operation_summary(op);
                enrich_error_responses(op);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn openapi_info_version_matches_crate() {
        let doc = ApiDoc::openapi();
        assert_eq!(
            doc.info.version,
            env!("CARGO_PKG_VERSION"),
            "OpenAPI info.version must stay aligned with CARGO_PKG_VERSION / VERSION"
        );
    }

    #[test]
    fn openapi_doc_builds_without_stack_overflow() {
        let doc = ApiDoc::openapi();
        assert!(!doc.paths.paths.is_empty());
    }

    #[test]
    fn openapi_has_same_origin_server_and_bearer_auth() {
        let doc = ApiDoc::openapi();
        let servers = doc.servers.as_ref().expect("servers");
        assert!(
            servers.iter().any(|s| s.url == "/"),
            "expected same-origin server /"
        );
        let schemes = &doc
            .components
            .as_ref()
            .expect("components")
            .security_schemes;
        assert!(
            schemes.contains_key("bearer_auth"),
            "expected bearer_auth security scheme"
        );
        let desc = doc.info.description.as_deref().unwrap_or("");
        assert!(
            desc.contains("bearer_auth") && desc.contains("vulyk_session"),
            "info.description should document Scalar Auth helper"
        );
    }

    #[test]
    fn openapi_registers_sso_webhooks_and_restore_upload() {
        let doc = ApiDoc::openapi();
        let paths = &doc.paths.paths;
        for path in [
            "/api/v1/auth/sso/{key}/start",
            "/api/v1/auth/sso/callback",
            "/api/v1/webhooks/github",
            "/api/v1/webhooks/github/{config_id}",
            "/api/v1/webhooks/gitlab",
            "/api/v1/webhooks/gitlab/{config_id}",
            "/api/v1/admin/backups/restore/upload",
        ] {
            assert!(
                paths.contains_key(path),
                "OpenAPI must register path {path}"
            );
        }
    }

    #[test]
    fn openapi_suite_results_documents_limit_offset() {
        let doc = ApiDoc::openapi();
        let path = doc
            .paths
            .paths
            .get("/api/v1/projects/{project_id}/suites-runs/{run_id}/results")
            .expect("suite results path");
        let get = path.get.as_ref().expect("GET operation");
        let names: Vec<&str> = get
            .parameters
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|p| match p {
                utoipa::openapi::RefOr::T(param) => Some(param.name.as_str()),
                utoipa::openapi::RefOr::Ref(_) => None,
            })
            .collect();
        assert!(
            names.contains(&"limit") && names.contains(&"offset"),
            "expected limit/offset query params, got {names:?}"
        );
    }

    #[test]
    fn openapi_every_operation_has_summary_and_problem_schema() {
        let doc = ApiDoc::openapi();
        let schemas = &doc.components.as_ref().expect("components").schemas;
        assert!(
            schemas.contains_key("ProblemDetails"),
            "ProblemDetails schema must be registered"
        );
        let responses = &doc.components.as_ref().unwrap().responses;
        for name in [
            "ProblemBadRequest",
            "ProblemUnauthorized",
            "ProblemForbidden",
            "ProblemNotFound",
        ] {
            assert!(
                responses.contains_key(name),
                "shared response component {name} missing"
            );
        }

        let mut missing = Vec::new();
        for (path, item) in &doc.paths.paths {
            for (method, op) in [
                ("GET", item.get.as_ref()),
                ("PUT", item.put.as_ref()),
                ("POST", item.post.as_ref()),
                ("DELETE", item.delete.as_ref()),
                ("PATCH", item.patch.as_ref()),
                ("HEAD", item.head.as_ref()),
                ("OPTIONS", item.options.as_ref()),
                ("TRACE", item.trace.as_ref()),
            ]
            .into_iter()
            .filter_map(|(m, o)| o.map(|op| (m, op)))
            {
                let summary = op.summary.as_deref().unwrap_or("").trim();
                if summary.is_empty() {
                    missing.push(format!("{method} {path}"));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "operations missing summary: {}",
            missing.join(", ")
        );
    }

    #[test]
    fn humanize_operation_id_capitalizes_words() {
        assert_eq!(humanize_operation_id("list_projects"), "List projects");
        assert_eq!(humanize_operation_id("get_case"), "Get case");
    }
}
