export const API_BASE =
  import.meta.env.VITE_API_URL ?? "http://localhost:8080/api/v1";

export type AuthTokens = {
  access_token: string;
  refresh_token: string;
  token_type: string;
};
export type User = { id: string; email: string; display_name: string | null };
export type Workspace = {
  id: string;
  name: string;
  slug: string;
  role: string;
};
export type Project = {
  id: string;
  workspace_id: string;
  name: string;
  slug: string;
  description: string | null;
  status: string;
};
export type OverviewPhase = {
  id: string;
  phase_type: string;
  position: number;
  status: string;
  required_criteria: number;
  completed_required_criteria: number;
};
export type ValidationCriterion = {
  id: string;
  phase_id: string;
  code: string;
  label: string;
  required: boolean;
  completed: boolean;
};
export type AnalyzeWorkspaceData = {
  problem: string;
  target_audiences: string[];
  target_details: string;
  value_proposition: string;
  success_objectives: string[];
  budget: string;
  deadline: string;
  platform: string;
  special_constraints: string;
  constraints_unknown: boolean;
  mvp_features: string[];
};
export type ModelAttribute = {
  conceptual_name: string;
  logical_name: string | null;
  physical_name: string | null;
  data_type: string | null;
  is_primary_key: boolean;
  is_unique: boolean;
  is_nullable: boolean;
  default_value: string | null;
};
export type ModelEntity = {
  conceptual_name: string;
  logical_name: string | null;
  physical_name: string | null;
  description: string | null;
  attributes: ModelAttribute[];
};
export type ModelRelationship = {
  name: string;
  source_entity: string;
  target_entity: string;
  source_cardinality: string;
  target_cardinality: string;
  description: string | null;
};
export type BusinessRule = { title: string; description: string };
export type ModelWorkspaceData = {
  entities: ModelEntity[];
  relationships: ModelRelationship[];
  business_rules: BusinessRule[];
  artifacts: string[];
};
export type DesignComponent = {
  name: string;
  category: "FRONTEND" | "BACKEND" | "DATABASE" | "EXTERNAL";
  responsibility: string;
  technology: string;
};
export type DesignWorkspaceData = {
  components: DesignComponent[];
  connections: {
    source_name: string;
    target_name: string;
    protocol: string;
    description: string;
  }[];
  ux_flows: unknown[];
  features: unknown[];
  api_contracts: unknown[];
  security_controls: unknown[];
  decisions: unknown[];
  artifacts: string[];
};
export type BuildWorkspaceData = {
  design_pack_ready: boolean;
  requirements: { id: string; code: string; title: string; status: string }[];
  tasks: {
    id: string;
    requirement_id: string | null;
    code: string;
    title: string;
    description: string | null;
    status: string;
    priority: string;
  }[];
  decisions: {
    id: string;
    code: string;
    title: string;
    decision: string;
    status: string;
  }[];
  github: {
    repository_url: string;
    default_branch: string;
    integration_strategy: "PULL_REQUEST" | "TRUNK_BASED" | "GIT_FLOW";
    ci_required: boolean;
    ci_configured: boolean;
    definition_of_done: unknown[];
  };
  progress: { total: number; done: number; blocked: number; percent: number };
  artifacts: string[];
};
export type TestWorkspaceData = {
  build_plan_ready: boolean;
  requirements: { id: string; code: string; title: string }[];
  tasks: { id: string; code: string; title: string }[];
  test_cases: {
    id: string; requirement_id: string | null; task_id: string | null; code: string;
    title: string; test_type: string; expected_result: string;
    actual_result: string | null; status: "NOT_RUN" | "PASSED" | "FAILED" | "BLOCKED";
  }[];
  defects: {
    id: string; test_case_id: string; title: string; description: string;
    severity: "BLOCKING" | "NON_BLOCKING"; status: "OPEN" | "RESOLVED" | "ACCEPTED";
    resolution: string | null;
  }[];
  progress: { total: number; executed: number; passed: number; failed: number; blocked: number; percent: number; pass_rate: number };
  artifacts: string[];
};
export type DeployWorkspaceData = {
  test_report_ready: boolean;
  profile: {
    target_environment: string;
    production_url: string;
    provider: string;
    deployment_status: "PREPARING" | "READY" | "DEPLOYED" | "FAILED" | "ROLLED_BACK";
    configuration_notes: string;
    configuration_keys: unknown[];
    migrations_required: boolean;
    migrations_plan: string;
    backups_required: boolean;
    backups_plan: string;
    monitoring_required: boolean;
    monitoring_plan: string;
    healthcheck_required: boolean;
    healthcheck: string;
    rollback_strategy: string;
    deployed_version: string;
    deployed_at: string | null;
    release_notes: string;
  };
  artifacts: string[];
};
export type ProjectOverview = {
  project: {
    id: string;
    name: string;
    description: string | null;
    status: string;
  };
  phases: OverviewPhase[];
  requirement_count: number;
  open_task_count: number;
  decision_count: number;
  latest_decision: { code: string; title: string; status: string } | null;
};

async function request<T>(
  path: string,
  options: RequestInit = {},
  token?: string,
): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const response = await fetch(`${API_BASE}${path}`, { ...options, headers });
  if (!response.ok) throw new Error(`HORUS API ${response.status}`);
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export const api = {
  login: (email: string, password: string) =>
    request<AuthTokens>("/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),
  register: (email: string, password: string, displayName: string) =>
    request<AuthTokens>("/auth/register", {
      method: "POST",
      body: JSON.stringify({
        email,
        password,
        display_name: displayName || null,
      }),
    }),
  me: (token: string) => request<User>("/auth/me", {}, token),
  projects: (token: string) => request<Project[]>("/projects", {}, token),
  workspaces: (token: string) => request<Workspace[]>("/workspaces", {}, token),
  overview: (projectId: string, token: string) =>
    request<ProjectOverview>(`/projects/${projectId}/overview`, {}, token),
  createProject: (
    workspaceId: string,
    name: string,
    description: string,
    token: string,
  ) =>
    request<Project>(
      "/projects",
      {
        method: "POST",
        body: JSON.stringify({
          workspace_id: workspaceId,
          name,
          description: description || null,
        }),
      },
      token,
    ),
  criteria: (phaseId: string, token: string) =>
    request<ValidationCriterion[]>(`/phases/${phaseId}/criteria`, {}, token),
  startPhase: (phaseId: string, token: string) =>
    request<void>(`/phases/${phaseId}/start`, { method: "POST" }, token),
  updateCriterion: (
    phaseId: string,
    criterionId: string,
    completed: boolean,
    token: string,
  ) =>
    request<void>(
      `/phases/${phaseId}/criteria/${criterionId}`,
      { method: "PATCH", body: JSON.stringify({ completed }) },
      token,
    ),
  validatePhase: (phaseId: string, token: string, comment?: string) =>
    request<void>(
      `/phases/${phaseId}/validate`,
      { method: "POST", body: JSON.stringify({ comment: comment || null }) },
      token,
    ),
  analyze: (phaseId: string, token: string) =>
    request<AnalyzeWorkspaceData>(`/phases/${phaseId}/analyze`, {}, token),
  saveAnalyze: (phaseId: string, value: AnalyzeWorkspaceData, token: string) =>
    request<AnalyzeWorkspaceData>(
      `/phases/${phaseId}/analyze`,
      { method: "PUT", body: JSON.stringify(value) },
      token,
    ),
  model: (phaseId: string, token: string) =>
    request<ModelWorkspaceData>(`/phases/${phaseId}/model`, {}, token),
  saveModel: (phaseId: string, model: ModelWorkspaceData, token: string) =>
    request<ModelWorkspaceData>(
      `/phases/${phaseId}/model`,
      { method: "PUT", body: JSON.stringify(model) },
      token,
    ),
  generateModelArtifact: (phaseId: string, level: string, token: string) =>
    request<unknown>(
      `/phases/${phaseId}/model/artifacts/${level}`,
      { method: "POST" },
      token,
    ),
  design: (phaseId: string, token: string) =>
    request<DesignWorkspaceData>(`/phases/${phaseId}/design`, {}, token),
  saveDesign: (phaseId: string, value: DesignWorkspaceData, token: string) =>
    request<DesignWorkspaceData>(
      `/phases/${phaseId}/design`,
      { method: "PUT", body: JSON.stringify(value) },
      token,
    ),
  generateDesignArtifact: (phaseId: string, kind: string, token: string) =>
    request<unknown>(
      `/phases/${phaseId}/design/artifacts/${kind}`,
      { method: "POST" },
      token,
    ),
  build: (phaseId: string, token: string) =>
    request<BuildWorkspaceData>(`/phases/${phaseId}/build`, {}, token),
  saveBuildGithub: (
    phaseId: string,
    github: BuildWorkspaceData["github"],
    token: string,
  ) =>
    request<BuildWorkspaceData>(
      `/phases/${phaseId}/build/github`,
      { method: "PUT", body: JSON.stringify(github) },
      token,
    ),
  createBuildTask: (
    phaseId: string,
    task: {
      requirement_id: string;
      title: string;
      description?: string;
      priority: string;
    },
    token: string,
  ) =>
    request<BuildWorkspaceData>(
      `/phases/${phaseId}/build/tasks`,
      { method: "POST", body: JSON.stringify(task) },
      token,
    ),
  updateBuildTask: (
    phaseId: string,
    taskId: string,
    status: string,
    token: string,
  ) =>
    request<BuildWorkspaceData>(
      `/phases/${phaseId}/build/tasks/${taskId}`,
      { method: "PATCH", body: JSON.stringify({ status }) },
      token,
    ),
  createBuildDecision: (
    phaseId: string,
    decision: { title: string; decision: string; status: string },
    token: string,
  ) =>
    request<BuildWorkspaceData>(
      `/phases/${phaseId}/build/decisions`,
      { method: "POST", body: JSON.stringify(decision) },
      token,
    ),
  generateBuildPlan: (phaseId: string, token: string) =>
    request<unknown>(
      `/phases/${phaseId}/build/artifacts/BUILD_PLAN`,
      { method: "POST" },
      token,
    ),
  testing: (phaseId: string, token: string) =>
    request<TestWorkspaceData>(`/phases/${phaseId}/test`, {}, token),
  createTestCase: (phaseId: string, value: { requirement_id?: string; task_id?: string; title: string; test_type: string; expected_result: string }, token: string) =>
    request<TestWorkspaceData>(`/phases/${phaseId}/test/cases`, { method: "POST", body: JSON.stringify(value) }, token),
  executeTestCase: (phaseId: string, caseId: string, status: string, actualResult: string, token: string) =>
    request<TestWorkspaceData>(`/phases/${phaseId}/test/cases/${caseId}`, { method: "PATCH", body: JSON.stringify({ status, actual_result: actualResult || null }) }, token),
  createTestDefect: (phaseId: string, value: { test_case_id: string; title: string; description: string; severity: string }, token: string) =>
    request<TestWorkspaceData>(`/phases/${phaseId}/test/defects`, { method: "POST", body: JSON.stringify(value) }, token),
  resolveTestDefect: (phaseId: string, defectId: string, resolution: string, token: string) =>
    request<TestWorkspaceData>(`/phases/${phaseId}/test/defects/${defectId}`, { method: "PATCH", body: JSON.stringify({ status: "RESOLVED", resolution }) }, token),
  generateTestReport: (phaseId: string, token: string) =>
    request<unknown>(`/phases/${phaseId}/test/artifacts/TEST_REPORT`, { method: "POST" }, token),
  deploy: (phaseId: string, token: string) =>
    request<DeployWorkspaceData>(`/phases/${phaseId}/deploy`, {}, token),
  saveDeploy: (phaseId: string, profile: DeployWorkspaceData["profile"], token: string) =>
    request<DeployWorkspaceData>(`/phases/${phaseId}/deploy`, { method: "PUT", body: JSON.stringify(profile) }, token),
  generateReleaseReport: (phaseId: string, token: string) =>
    request<unknown>(`/phases/${phaseId}/deploy/artifacts/RELEASE_REPORT`, { method: "POST" }, token),
};
