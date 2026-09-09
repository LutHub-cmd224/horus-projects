import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AnalyzeWorkspace } from "./AnalyzeWorkspace";
import { api, type AnalyzeWorkspaceData, type OverviewPhase, type ValidationCriterion } from "./api";

const phase: OverviewPhase = { id: "analyze", phase_type: "ANALYZE", position: 1, status: "AVAILABLE", required_criteria: 6, completed_required_criteria: 0 };
const profile: AnalyzeWorkspaceData = { problem: "", target_audiences: [], target_details: "", value_proposition: "", success_objectives: [], budget: "", deadline: "", platform: "", special_constraints: "", constraints_unknown: false, mvp_features: [] };
const criteria: ValidationCriterion[] = ["problem_defined","target_user_defined","value_proposition_defined","objectives_defined","constraints_defined","mvp_defined"].map((code,index) => ({ id:String(index),phase_id:"analyze",code,label:code,required:true,completed:false }));

describe("AnalyzeWorkspace guided journey", () => {
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });
  it("asks a plain-language question and saves the answer without manual criteria", async () => {
    vi.spyOn(api,"analyze").mockResolvedValue(profile);
    vi.spyOn(api,"criteria").mockResolvedValue(criteria);
    const save = vi.spyOn(api,"saveAnalyze").mockImplementation(async (_id,value) => value);
    const updateCriterion = vi.spyOn(api,"updateCriterion");
    render(<AnalyzeWorkspace onChanged={vi.fn()} phase={phase} token="token"/>);
    fireEvent.change(await screen.findByLabelText(/problème à résoudre/i), { target:{ value:"Je perds le fil de mes projets" } });
    fireEvent.click(screen.getByRole("button",{name:/enregistrer et continuer/i}));
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0][1].problem).toContain("perds le fil");
    expect(updateCriterion).not.toHaveBeenCalled();
    expect(await screen.findByText(/pour qui construis-tu/i)).toBeInTheDocument();
  });
  it("explains the next missing action", async () => {
    vi.spyOn(api,"analyze").mockResolvedValue(profile);
    vi.spyOn(api,"criteria").mockResolvedValue(criteria);
    render(<AnalyzeWorkspace onChanged={vi.fn()} phase={phase} token="token"/>);
    expect(await screen.findByText(/décrire le problème/i)).toBeInTheDocument();
  });
});
