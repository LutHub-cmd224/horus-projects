import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type AnalyzeWorkspaceData, type ProjectOverview } from "./api";
import { HorusJourney } from "./HorusJourney";

const profile: AnalyzeWorkspaceData = { problem:"Un problème connu",target_audiences:[],target_details:"",value_proposition:"",success_objectives:[],budget:"",deadline:"",platform:"",special_constraints:"",constraints_unknown:false,mvp_features:[] };
const overview: ProjectOverview = {
  project:{id:"project",name:"PRISM",description:"Mettre les bonnes personnes en relation",status:"ACTIVE"},
  phases:[
    {id:"analyze",phase_type:"ANALYZE",position:1,status:"AVAILABLE",required_criteria:6,completed_required_criteria:1},
    ...["MODEL","DESIGN","BUILD","TEST","DEPLOY"].map((phase_type,index)=>({id:phase_type.toLowerCase(),phase_type,position:index+2,status:"LOCKED",required_criteria:6,completed_required_criteria:0})),
  ],
  requirement_count:0,open_task_count:0,decision_count:0,latest_decision:null,
  next_action:{phase:"ANALYZE",phase_id:"analyze",step:"target_user_defined",title:"Définir les utilisateurs principaux",cta:"Continuer",reason:"HORUS doit savoir pour qui le produit est construit.",blocked:false},
};
const projects=[{...overview.project,workspace_id:"workspace",slug:"prism"}];

describe("HORUS Journey navigation",()=>{
  afterEach(()=>{cleanup();vi.restoreAllMocks();});
  it("resumes a project on the exact question selected by the server",async()=>{
    vi.spyOn(api,"analyze").mockResolvedValue(profile);
    render(<HorusJourney onExit={vi.fn()} onRefresh={vi.fn()} onSelectProject={vi.fn()} overview={overview} projects={projects} selectedProjectId="project" token="token" user={{id:"user",email:"user@example.test",display_name:"Luther"}}/>);
    expect(screen.getByRole("heading",{name:/reprends ton idée/i})).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:/continuer prism/i}));
    expect(await screen.findByText(/pour qui construis-tu/i)).toBeInTheDocument();
    expect(screen.queryByText(/quel problème veux-tu principalement/i)).not.toBeInTheDocument();
  });

  it("keeps the critical navigation usable on a narrow viewport",()=>{
    Object.defineProperty(window,"innerWidth",{configurable:true,value:375});
    render(<HorusJourney onExit={vi.fn()} onRefresh={vi.fn()} onSelectProject={vi.fn()} overview={overview} projects={projects} selectedProjectId="project" token="token" user={null}/>);
    expect(screen.getByRole("button",{name:/continuer prism/i})).toBeVisible();
    expect(screen.getByRole("button",{name:"Journey"})).toBeVisible();
  });
});
