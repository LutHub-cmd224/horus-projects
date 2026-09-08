import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type DesignWorkspaceData } from "./api";
import { DesignWorkspace } from "./DesignWorkspace";
const data: DesignWorkspaceData = {
  components: [],
  connections: [],
  ux_flows: [],
  features: [],
  api_contracts: [],
  security_controls: [],
  decisions: [],
  artifacts: [],
};
const phase = {
  id: "design",
  phase_type: "DESIGN",
  position: 3,
  status: "AVAILABLE",
  required_criteria: 7,
  completed_required_criteria: 0,
};
describe("DesignWorkspace", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });
  it("loads, edits and saves architecture", async () => {
    vi.spyOn(api, "design").mockResolvedValue(data);
    const save = vi
      .spyOn(api, "saveDesign")
      .mockImplementation(async (_id, v) => v);
    render(<DesignWorkspace onChanged={vi.fn()} phase={phase} token="token" />);
    fireEvent.click(
      (await screen.findAllByRole("button", { name: /^\+$/i }))[0],
    );
    fireEvent.click(screen.getByRole("button", { name: /enregistrer/i }));
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0][1].components).toHaveLength(1);
  });
  it("validates a generated design pack", async () => {
    vi.spyOn(api, "design").mockResolvedValue({
      ...data,
      decisions: [{ status: "ACCEPTED" }],
      artifacts: ["DESIGN_PACK"],
    });
    const validate = vi
      .spyOn(api, "validatePhase")
      .mockResolvedValue(undefined);
    render(
      <DesignWorkspace
        onChanged={vi.fn()}
        phase={{ ...phase, status: "IN_PROGRESS" }}
        token="token"
      />,
    );
    fireEvent.click(
      await screen.findByRole("button", { name: /valider concevoir/i }),
    );
    await waitFor(() =>
      expect(validate).toHaveBeenCalledWith(
        "design",
        "token",
        "Validation du Design Pack",
      ),
    );
  });
  it("requires an accepted decision before generating ADR artifacts", async () => {
    vi.spyOn(api, "design").mockResolvedValue({
      ...data,
      decisions: [{ title: "ADR-001", status: "PROPOSED" }],
    });
    const { rerender } = render(
      <DesignWorkspace onChanged={vi.fn()} phase={phase} token="token" />,
    );
    expect(
      await screen.findByRole("button", { name: "ADR_INDEX" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "DESIGN_PACK" }),
    ).toBeDisabled();

    vi.mocked(api.design).mockResolvedValue({
      ...data,
      decisions: [{ title: "ADR-001", status: "ACCEPTED" }],
    });
    rerender(
      <DesignWorkspace onChanged={vi.fn()} phase={{ ...phase }} token="next" />,
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "ADR_INDEX" }),
      ).not.toBeDisabled(),
    );
    expect(
      screen.getByRole("button", { name: "DESIGN_PACK" }),
    ).not.toBeDisabled();
  });
});
