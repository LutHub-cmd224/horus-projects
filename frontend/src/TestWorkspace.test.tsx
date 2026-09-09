import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type TestWorkspaceData } from "./api";
import { TestWorkspace } from "./TestWorkspace";

const data: TestWorkspaceData = {
  build_plan_ready: true,
  requirements: [{ id:"req", code:"REQ-001", title:"Quality" }],
  tasks: [{ id:"task", code:"TASK-001", title:"Implement quality" }],
  test_cases: [], defects: [],
  progress: { total:0, executed:0, passed:0, failed:0, blocked:0, percent:0, pass_rate:0 },
  artifacts: [],
};
const phase = { id:"test", phase_type:"TEST", position:5, status:"AVAILABLE", required_criteria:7, completed_required_criteria:1 };

describe("TestWorkspace",()=>{
  afterEach(()=>{cleanup();vi.restoreAllMocks()});
  it("creates a traced test case and records its result",async()=>{
    vi.spyOn(api,"testing").mockResolvedValue(data);
    const created={...data,test_cases:[{id:"case",requirement_id:"req",task_id:null,code:"TEST-001",title:"Quality works",test_type:"E2E",expected_result:"It works",actual_result:null,status:"NOT_RUN" as const}],progress:{...data.progress,total:1}};
    const create=vi.spyOn(api,"createTestCase").mockResolvedValue(created);
    const execute=vi.spyOn(api,"executeTestCase").mockResolvedValue({...created,test_cases:[{...created.test_cases[0],status:"PASSED",actual_result:"Conforme"}],progress:{total:1,executed:1,passed:1,failed:0,blocked:0,percent:100,pass_rate:100}});
    render(<TestWorkspace onChanged={vi.fn()} phase={phase} token="token"/>);
    fireEvent.change(await screen.findByPlaceholderText("Scénario"),{target:{value:"Quality works"}});
    fireEvent.change(screen.getByPlaceholderText("Résultat attendu"),{target:{value:"It works"}});
    fireEvent.click(screen.getByRole("button",{name:"Ajouter"}));
    await waitFor(()=>expect(create).toHaveBeenCalledWith("test",expect.objectContaining({requirement_id:"req",test_type:"UNIT"}),"token"));
    fireEvent.change(await screen.findByRole("combobox",{name:"Résultat TEST-001"}),{target:{value:"PASSED"}});
    await waitFor(()=>expect(execute).toHaveBeenCalledWith("test","case","PASSED","Conforme","token"));
    expect(await screen.findByText("100% · 100%")).toBeInTheDocument();
  });
  it("locks editing after validation",async()=>{
    vi.spyOn(api,"testing").mockResolvedValue({...data,artifacts:["TEST_REPORT"]});
    render(<TestWorkspace onChanged={vi.fn()} phase={{...phase,status:"VALIDATED"}} token="token"/>);
    expect(await screen.findByPlaceholderText("Scénario")).toBeDisabled();
    expect(screen.getByRole("button",{name:/rapport/i})).toBeDisabled();
  });
  it("creates a blocking defect from a blocked test",async()=>{
    const blocked={...data,test_cases:[{id:"case",requirement_id:"req",task_id:"task",code:"TEST-001",title:"Deployment gate",test_type:"E2E",expected_result:"Gate opens",actual_result:"Gate unavailable",status:"BLOCKED" as const}],progress:{total:1,executed:1,passed:0,failed:0,blocked:1,percent:100,pass_rate:0}};
    vi.spyOn(api,"testing").mockResolvedValue(blocked);
    const defect=vi.spyOn(api,"createTestDefect").mockResolvedValue({...blocked,defects:[{id:"defect",test_case_id:"case",title:"Anomalie TEST-001",description:"Gate unavailable",severity:"BLOCKING",status:"OPEN",resolution:null}]});
    render(<TestWorkspace onChanged={vi.fn()} phase={phase} token="token"/>);
    fireEvent.click(await screen.findByRole("button",{name:"Créer une anomalie"}));
    await waitFor(()=>expect(defect).toHaveBeenCalledWith("test",expect.objectContaining({test_case_id:"case",severity:"BLOCKING"}),"token"));
    expect(await screen.findByText(/Anomalie TEST-001/)).toBeInTheDocument();
  });
});
