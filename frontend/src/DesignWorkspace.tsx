import { useEffect, useState } from "react";
import { api, type DesignWorkspaceData, type OverviewPhase } from "./api";
const empty: DesignWorkspaceData = {
  components: [],
  connections: [],
  ux_flows: [],
  features: [],
  api_contracts: [],
  security_controls: [],
  decisions: [],
  artifacts: [],
};
const sections = [
  ["ux_flows", "Parcours UX"],
  ["features", "Fonctionnalités"],
  ["api_contracts", "Contrats API"],
  ["security_controls", "Sécurité"],
  ["decisions", "Décisions d’architecture"],
] as const;
const hasAcceptedDecision = (decisions: unknown[]) =>
  decisions.some(
    (decision) =>
      typeof decision === "object" &&
      decision !== null &&
      "status" in decision &&
      decision.status === "ACCEPTED",
  );
export function DesignWorkspace({
  phase,
  token,
  onChanged,
}: {
  phase: OverviewPhase;
  token: string;
  onChanged: () => Promise<void>;
}) {
  const [d, setD] = useState(empty),
    [busy, setBusy] = useState(""),
    [error, setError] = useState("");
  useEffect(() => {
    let c = false;
    api
      .design(phase.id, token)
      .then((v) => {
        if (!c) setD(v);
      })
      .catch(() => {
        if (!c) setError("Impossible de charger la conception.");
      });
    return () => {
      c = true;
    };
  }, [phase.id, token]);
  const locked = phase.status === "VALIDATED";
  async function save() {
    setBusy("save");
    try {
      setD(await api.saveDesign(phase.id, d, token));
      await onChanged();
    } catch {
      setError("Conception invalide ou incomplète.");
    } finally {
      setBusy("");
    }
  }
  async function artifact(k: string) {
    setBusy(k);
    try {
      await api.generateDesignArtifact(phase.id, k, token);
      setD(await api.design(phase.id, token));
      await onChanged();
    } catch {
      setError(`${k} ne peut pas encore être généré.`);
    } finally {
      setBusy("");
    }
  }
  async function validate() {
    setBusy("validate");
    try {
      await api.validatePhase(phase.id, token, "Validation du Design Pack");
      await onChanged();
    } catch {
      setError("Le Design Pack doit être complet.");
    } finally {
      setBusy("");
    }
  }
  function setDecisionStatus(index: number, status: string) {
    setD((value) => ({
      ...value,
      decisions: value.decisions.map((decision, decisionIndex) =>
        decisionIndex === index &&
        typeof decision === "object" &&
        decision !== null
          ? { ...decision, status }
          : decision,
      ),
    }));
  }
  return (
    <section
      className="mt-14 rounded-[2rem] border border-black/10 bg-white/55 p-6 sm:p-8"
      id="design-workspace"
    >
      <p className="text-xs font-semibold uppercase tracking-[.2em] text-[#d9503f]">
        Phase 03 · Concevoir
      </p>
      <div className="mt-3 flex justify-between gap-4">
        <div>
          <h2 className="text-4xl font-semibold">Concevoir avant de coder.</h2>
          <p className="mt-2 text-sm text-black/50">
            Architecture, UX, fonctionnalités, API, sécurité et décisions
            techniques.
          </p>
        </div>
        <button
          className="rounded-full bg-black px-6 py-3 text-sm font-bold text-white disabled:opacity-40"
          disabled={locked || !!busy}
          onClick={() => void save()}
        >
          Enregistrer
        </button>
      </div>
      <div className="mt-8 grid gap-4 lg:grid-cols-2">
        <article className="rounded-2xl border border-black/10 p-5">
          <div className="flex justify-between">
            <h3 className="font-semibold">Architecture technique</h3>
            <button
              disabled={locked}
              onClick={() =>
                setD((v) => ({
                  ...v,
                  components: [
                    ...v.components,
                    {
                      name: `Composant ${v.components.length + 1}`,
                      category: "BACKEND",
                      responsibility: "Responsabilité à préciser",
                      technology: "Technologie",
                    },
                  ],
                }))
              }
            >
              +
            </button>
          </div>
          {d.components.map((c, i) => (
            <div className="mt-3 grid gap-2 sm:grid-cols-2" key={i}>
              <input
                className="rounded-lg border p-2 text-sm"
                value={c.name}
                onChange={(e) =>
                  setD((v) => ({
                    ...v,
                    components: v.components.map((x, j) =>
                      j === i ? { ...x, name: e.target.value } : x,
                    ),
                  }))
                }
              />
              <input
                className="rounded-lg border p-2 text-sm"
                value={c.technology}
                onChange={(e) =>
                  setD((v) => ({
                    ...v,
                    components: v.components.map((x, j) =>
                      j === i ? { ...x, technology: e.target.value } : x,
                    ),
                  }))
                }
              />
            </div>
          ))}
        </article>
        {sections.map(([key, label]) => (
          <article className="rounded-2xl border border-black/10 p-5" key={key}>
            <div className="flex justify-between">
              <h3 className="font-semibold">{label}</h3>
              <button
                disabled={locked}
                onClick={() =>
                  setD((v) => ({
                    ...v,
                    [key]: [
                      ...v[key],
                      {
                        title: `${label} ${v[key].length + 1}`,
                        description: "À préciser",
                        ...(key === "decisions" ? { status: "PROPOSED" } : {}),
                      },
                    ],
                  }))
                }
              >
                +
              </button>
            </div>
            <pre className="mt-3 max-h-32 overflow-auto whitespace-pre-wrap text-xs text-black/50">
              {JSON.stringify(d[key], null, 2)}
            </pre>
            {key === "decisions" &&
              d.decisions.map((decision, index) => (
                <select
                  aria-label={`Statut décision ${index + 1}`}
                  className="mt-2 w-full rounded-lg border p-2 text-sm"
                  disabled={locked}
                  key={index}
                  onChange={(event) =>
                    setDecisionStatus(index, event.target.value)
                  }
                  value={
                    typeof decision === "object" &&
                    decision !== null &&
                    "status" in decision &&
                    typeof decision.status === "string"
                      ? decision.status
                      : "PROPOSED"
                  }
                >
                  <option value="PROPOSED">Proposée</option>
                  <option value="ACCEPTED">Acceptée</option>
                  <option value="REJECTED">Rejetée</option>
                  <option value="SUPERSEDED">Remplacée</option>
                </select>
              ))}
          </article>
        ))}
      </div>
      {error && (
        <p className="mt-4 rounded-xl bg-red-50 p-3 text-sm text-red-700">
          {error}
        </p>
      )}
      <div className="mt-6 flex flex-wrap gap-2">
        {[
          "ARCHITECTURE",
          "UX",
          "FEATURES",
          "API_CONTRACTS",
          "SECURITY",
          "ADR_INDEX",
          "DESIGN_PACK",
        ].map((k) => (
          <button
            className={`rounded-full border px-4 py-2 text-xs font-semibold ${d.artifacts.includes(k) ? "bg-black text-white" : ""}`}
            disabled={
              locked ||
              !!busy ||
              ((k === "ADR_INDEX" || k === "DESIGN_PACK") &&
                !hasAcceptedDecision(d.decisions))
            }
            onClick={() => void artifact(k)}
            key={k}
          >
            {d.artifacts.includes(k) ? "✓ " : ""}
            {k}
          </button>
        ))}
        <button
          className="ml-auto rounded-full bg-[#d9503f] px-5 py-2 text-sm font-bold text-white disabled:opacity-35"
          disabled={locked || !!busy || !d.artifacts.includes("DESIGN_PACK")}
          onClick={() => void validate()}
        >
          {locked ? "Phase validée" : "Valider Concevoir →"}
        </button>
      </div>
    </section>
  );
}
