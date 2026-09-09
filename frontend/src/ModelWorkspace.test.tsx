import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type ModelWorkspaceData, type OverviewPhase, type ValidationCriterion } from "./api";
import { ModelWorkspace } from "./ModelWorkspace";

const phase: OverviewPhase = { id:"model",phase_type:"MODEL",position:2,status:"AVAILABLE",required_criteria:6,completed_required_criteria:0 };
const criteria: ValidationCriterion[] = ["entities_defined","relations_defined","business_rules_defined","mcd_defined","mld_defined","mpd_defined"].map((code,index) => ({id:String(index),phase_id:"model",code,label:code,required:true,completed:false}));
const empty: ModelWorkspaceData = {entities:[],relationships:[],business_rules:[],artifacts:[]};

describe("ModelWorkspace guided journey", () => {
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });
  it("collects simple names while leaving technical fields empty", async () => {
    vi.spyOn(api,"model").mockResolvedValue(empty);
    vi.spyOn(api,"criteria").mockResolvedValue(criteria);
    const save = vi.spyOn(api,"saveModel").mockImplementation(async (_id,value) => value);
    render(<ModelWorkspace onChanged={vi.fn()} phase={phase} token="token"/>);
    fireEvent.click(await screen.findByRole("button",{name:/ajouter quelque chose/i}));
    fireEvent.change(screen.getByLabelText(/nom de l'élément 1/i),{target:{value:"Utilisateur"}});
    fireEvent.click(screen.getByRole("button",{name:/ajouter une information/i}));
    fireEvent.change(screen.getByLabelText(/information 1/i),{target:{value:"email"}});
    fireEvent.click(screen.getByRole("button",{name:/enregistrer et continuer/i}));
    await waitFor(() => expect(save).toHaveBeenCalled());
    const sent = save.mock.calls[0][1].entities[0];
    expect(sent.logical_name).toBeNull();
    expect(sent.physical_name).toBeNull();
    expect(sent.attributes[0].data_type).toBeNull();
  });
  it("presents technical generation through understandable actions", async () => {
    vi.spyOn(api,"model").mockResolvedValue({...empty,artifacts:["MCD","MLD"]});
    vi.spyOn(api,"criteria").mockResolvedValue(criteria);
    vi.spyOn(api,"saveModel").mockImplementation(async (_id,value) => value);
    render(<ModelWorkspace onChanged={vi.fn()} phase={phase} token="token"/>);
    fireEvent.click(await screen.findByRole("button",{name:/enregistrer et continuer/i}));
    await screen.findByText(/comment ces éléments/i);
    fireEvent.click(screen.getByRole("button",{name:/enregistrer et continuer/i}));
    await screen.findByText(/règles importantes/i);
    fireEvent.click(screen.getByRole("button",{name:/enregistrer et continuer/i}));
    expect(await screen.findByRole("button",{name:/préparer la structure de la base de données/i})).toHaveTextContent("MPD");
  });
});
