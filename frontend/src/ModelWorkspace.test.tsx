import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type ModelWorkspaceData, type OverviewPhase } from "./api";
import { ModelWorkspace } from "./ModelWorkspace";

const phase: OverviewPhase = { id:"model",phase_type:"MODEL",position:2,status:"AVAILABLE",required_criteria:6,completed_required_criteria:0 };
const empty: ModelWorkspaceData = {entities:[],relationships:[],business_rules:[],artifacts:[]};

describe("ModelWorkspace guided journey", () => {
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });
  it("collects simple names while leaving technical fields empty", async () => {
    vi.spyOn(api,"model").mockResolvedValue(empty);
    const save = vi.spyOn(api,"saveModel").mockImplementation(async (_id,value) => value);
    render(<ModelWorkspace onChanged={vi.fn()} phase={phase} token="token"/>);
    fireEvent.click(await screen.findByRole("button",{name:/tout garder/i}));
    fireEvent.click(screen.getByRole("button",{name:/confirmer et continuer/i}));
    await waitFor(() => expect(save).toHaveBeenCalled());
    const sent = save.mock.calls[0][1].entities[0];
    expect(sent.logical_name).toBeNull();
    expect(sent.physical_name).toBeNull();
    expect(sent.attributes[0].data_type).toBeNull();
  });
  it("presents technical generation through understandable actions", async () => {
    vi.spyOn(api,"model").mockResolvedValue({...empty,artifacts:["MCD","MLD"]});
    vi.spyOn(api,"saveModel").mockImplementation(async (_id,value) => value);
    render(<ModelWorkspace initialStep="mcd_defined" onChanged={vi.fn()} phase={phase} token="token"/>);
    expect(await screen.findByRole("button",{name:/préparer la structure complète/i})).toBeInTheDocument();
    fireEvent.click(screen.getByText(/voir les détails techniques/i));
    expect(screen.getByText(/✓ MCD/)).toBeInTheDocument();
    expect(screen.getByText(/✓ MLD/)).toBeInTheDocument();
  });
});
