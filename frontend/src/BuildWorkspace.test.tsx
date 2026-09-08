import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type BuildWorkspaceData } from "./api";
import { BuildWorkspace } from "./BuildWorkspace";

const data: BuildWorkspaceData = {
  design_pack_ready: true,
  requirements: [
    { id: "req-1", code: "REQ-001", title: "Traceability", status: "APPROVED" },
  ],
  tasks: [],
  decisions: [],
  github: {
    repository_url: "",
    default_branch: "main",
    integration_strategy: "PULL_REQUEST",
    ci_required: true,
    definition_of_done: [],
  },
  progress: { total: 0, done: 0, blocked: 0, percent: 0 },
  artifacts: [],
};
const phase = {
  id: "build",
  phase_type: "BUILD",
  position: 4,
  status: "AVAILABLE",
  required_criteria: 6,
  completed_required_criteria: 1,
};

describe("BuildWorkspace", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("creates a backlog task linked to a requirement", async () => {
    vi.spyOn(api, "build").mockResolvedValue(data);
    const create = vi.spyOn(api, "createBuildTask").mockResolvedValue({
      ...data,
      tasks: [
        {
          id: "task-1",
          requirement_id: "req-1",
          code: "TASK-001",
          title: "Implement traceability",
          description: null,
          status: "TODO",
          priority: "MEDIUM",
        },
      ],
      progress: { total: 1, done: 0, blocked: 0, percent: 0 },
    });
    render(<BuildWorkspace onChanged={vi.fn()} phase={phase} token="token" />);
    fireEvent.change(await screen.findByPlaceholderText("Nouvelle tâche"), {
      target: { value: "Implement traceability" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Ajouter au backlog" }));
    await waitFor(() =>
      expect(create).toHaveBeenCalledWith(
        "build",
        expect.objectContaining({ requirement_id: "req-1" }),
        "token",
      ),
    );
    expect(await screen.findByText("Implement traceability")).toBeInTheDocument();
  });

  it("keeps Build read-only once the phase is validated", async () => {
    vi.spyOn(api, "build").mockResolvedValue({ ...data, artifacts: ["BUILD_PLAN"] });
    render(
      <BuildWorkspace
        onChanged={vi.fn()}
        phase={{ ...phase, status: "VALIDATED" }}
        token="token"
      />,
    );
    expect(await screen.findByPlaceholderText("Nouvelle tâche")).toBeDisabled();
    expect(screen.getByRole("button", { name: /Build Plan/ })).toBeDisabled();
  });
});
