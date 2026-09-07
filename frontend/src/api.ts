export const API_BASE = import.meta.env.VITE_API_URL ?? 'http://localhost:8080/api/v1';

export type AuthTokens = {
  access_token: string;
  refresh_token: string;
  token_type: string;
};

export type User = {
  id: string;
  email: string;
  display_name: string | null;
};

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

async function request<T>(path: string, options: RequestInit = {}, token?: string): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set('Content-Type', 'application/json');
  if (token) headers.set('Authorization', `Bearer ${token}`);

  const response = await fetch(`${API_BASE}${path}`, { ...options, headers });
  if (!response.ok) throw new Error(`HORUS API ${response.status}`);
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export const api = {
  login: (email: string, password: string) =>
    request<AuthTokens>('/auth/login', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    }),
  register: (email: string, password: string, displayName: string) =>
    request<AuthTokens>('/auth/register', {
      method: 'POST',
      body: JSON.stringify({ email, password, display_name: displayName || null }),
    }),
  me: (token: string) => request<User>('/auth/me', {}, token),
  projects: (token: string) => request<Project[]>('/projects', {}, token),
  workspaces: (token: string) => request<Workspace[]>('/workspaces', {}, token),
  overview: (projectId: string, token: string) =>
    request<ProjectOverview>(`/projects/${projectId}/overview`, {}, token),
  createProject: (workspaceId: string, name: string, description: string, token: string) =>
    request<Project>(
      '/projects',
      {
        method: 'POST',
        body: JSON.stringify({
          workspace_id: workspaceId,
          name,
          description: description || null,
        }),
      },
      token,
    ),
};
