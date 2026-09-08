import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type ModelWorkspaceData, type OverviewPhase } from "./api";
import { ModelWorkspace } from "./ModelWorkspace";

const phase: OverviewPhase = { id: "model", phase_type: "MODEL", position: 2, status: "AVAILABLE", required_criteria: 6, completed_required_criteria: 0 };
const data: ModelWorkspaceData = { entities: [], relationships: [], business_rules: [], artifacts: [] };

describe("ModelWorkspace", () => {
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("loads and saves a new entity", async () => {
    vi.spyOn(api, "model").mockResolvedValue(data);
    const save = vi.spyOn(api, "saveModel").mockImplementation(async (_id, value) => value);
    render(<ModelWorkspace onChanged={vi.fn()} phase={phase} token="token" />);
    await screen.findByText(/entités et attributs/i);
    fireEvent.click(screen.getByRole("button", { name: /entité/i }));
    fireEvent.click(screen.getByRole("button", { name: /enregistrer/i }));
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0][1].entities).toHaveLength(1);
  });

  it("generates model artifacts", async () => {
    vi.spyOn(api, "model").mockResolvedValue(data);
    const generate = vi.spyOn(api, "generateModelArtifact").mockResolvedValue({});
    render(<ModelWorkspace onChanged={vi.fn()} phase={phase} token="token" />);
    fireEvent.click(await screen.findByRole("button", { name: /générer mcd/i }));
    await waitFor(() => expect(generate).toHaveBeenCalledWith("model", "MCD", "token"));
  });
});
