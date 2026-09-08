import { useEffect, useState, type FormEvent } from "react";
import { api, type BuildWorkspaceData, type OverviewPhase } from "./api";

const empty: BuildWorkspaceData = {
  design_pack_ready: false,
  requirements: [],
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

export function BuildWorkspace({
  phase,
  token,
  onChanged,
}: {
  phase: OverviewPhase;
  token: string;
  onChanged: () => Promise<void>;
}) {
  const [data, setData] = useState(empty);
  const [taskTitle, setTaskTitle] = useState("");
  const [requirementId, setRequirementId] = useState("");
  const [decisionTitle, setDecisionTitle] = useState("");
  const [decisionText, setDecisionText] = useState("");
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const locked = phase.status === "LOCKED" || phase.status === "VALIDATED";

  useEffect(() => {
    let cancelled = false;
    api
      .build(phase.id, token)
      .then((value) => {
        if (!cancelled) {
          setData(value);
          setRequirementId(value.requirements[0]?.id ?? "");
        }
      })
      .catch(() => !cancelled && setError("Impossible de charger le Build."));
    return () => {
      cancelled = true;
    };
  }, [phase.id, token]);

  async function run(label: string, action: () => Promise<BuildWorkspaceData>) {
    setBusy(label);
    setError("");
    try {
      setData(await action());
      await onChanged();
    } catch {
      setError("Action impossible : vérifiez les prérequis du Build.");
    } finally {
      setBusy("");
    }
  }

  async function addTask(event: FormEvent) {
    event.preventDefault();
    if (!requirementId) return;
    await run("task", () =>
      api.createBuildTask(
        phase.id,
        {
          requirement_id: requirementId,
          title: taskTitle,
          priority: "MEDIUM",
        },
        token,
      ),
    );
    setTaskTitle("");
  }

  async function addDecision(event: FormEvent) {
    event.preventDefault();
    await run("decision", () =>
      api.createBuildDecision(
        phase.id,
        { title: decisionTitle, decision: decisionText, status: "ACCEPTED" },
        token,
      ),
    );
    setDecisionTitle("");
    setDecisionText("");
  }

  const definitionOfDone = data.github.definition_of_done
    .map((item) => String(item))
    .join("\n");

  return (
    <section
      className="mt-14 rounded-[2rem] border border-black/10 bg-white/55 p-6 sm:p-8"
      id="build-workspace"
    >
      <p className="text-xs font-semibold uppercase tracking-[.2em] text-[#d9503f]">
        Phase 04 · Coder
      </p>
      <div className="mt-3 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h2 className="text-4xl font-semibold">Piloter l’implémentation.</h2>
          <p className="mt-2 text-sm text-black/50">
            Design Pack → backlog traçable → exécution GitHub.
          </p>
        </div>
        <div className="min-w-48">
          <div className="flex justify-between text-xs font-semibold">
            <span>{data.progress.done}/{data.progress.total} terminées</span>
            <span>{data.progress.percent}%</span>
          </div>
          <div className="mt-2 h-2 overflow-hidden rounded-full bg-black/10">
            <div
              className="h-full bg-[#d9503f]"
              style={{ width: `${data.progress.percent}%` }}
            />
          </div>
        </div>
      </div>
      {!data.design_pack_ready && (
        <p className="mt-5 rounded-xl bg-amber-50 p-3 text-sm text-amber-800">
          Le Design Pack validé est requis avant de générer le Build Plan.
        </p>
      )}
      <div className="mt-8 grid gap-5 lg:grid-cols-2">
        <article className="rounded-2xl border border-black/10 p-5">
          <h3 className="font-semibold">Backlog d’implémentation</h3>
          <form className="mt-4 grid gap-2" onSubmit={(e) => void addTask(e)}>
            <input className="rounded-lg border p-2 text-sm" disabled={locked} onChange={(e) => setTaskTitle(e.target.value)} placeholder="Nouvelle tâche" required value={taskTitle} />
            <select className="rounded-lg border p-2 text-sm" disabled={locked} onChange={(e) => setRequirementId(e.target.value)} required value={requirementId}>
              <option value="">Exigence liée…</option>
              {data.requirements.map((requirement) => <option key={requirement.id} value={requirement.id}>{requirement.code} — {requirement.title}</option>)}
            </select>
            <button className="rounded-lg bg-black px-4 py-2 text-sm font-bold text-white disabled:opacity-40" disabled={locked || !!busy}>Ajouter au backlog</button>
          </form>
          <div className="mt-4 space-y-2">
            {data.tasks.map((task) => <div className="flex items-center gap-3 rounded-xl bg-black/[.035] p-3" key={task.id}><div className="min-w-0 flex-1"><p className="text-xs text-black/40">{task.code} · {task.priority}</p><p className="truncate text-sm font-semibold">{task.title}</p></div><select aria-label={`Statut ${task.code}`} className="rounded-lg border p-2 text-xs" disabled={locked || !!busy} onChange={(e) => void run("status", () => api.updateBuildTask(phase.id, task.id, e.target.value, token))} value={task.status}>{["TODO","IN_PROGRESS","BLOCKED","DONE","CANCELLED"].map((status) => <option key={status}>{status}</option>)}</select></div>)}
          </div>
        </article>
        <article className="rounded-2xl border border-black/10 p-5">
          <h3 className="font-semibold">Préparation GitHub</h3>
          <div className="mt-4 grid gap-2">
            <input aria-label="Dépôt GitHub" className="rounded-lg border p-2 text-sm" disabled={locked} onChange={(e) => setData((v) => ({ ...v, github: { ...v.github, repository_url: e.target.value } }))} placeholder="https://github.com/org/repo" value={data.github.repository_url} />
            <input aria-label="Branche par défaut" className="rounded-lg border p-2 text-sm" disabled={locked} onChange={(e) => setData((v) => ({ ...v, github: { ...v.github, default_branch: e.target.value } }))} value={data.github.default_branch} />
            <textarea aria-label="Definition of Done" className="min-h-24 rounded-lg border p-2 text-sm" disabled={locked} onChange={(e) => setData((v) => ({ ...v, github: { ...v.github, definition_of_done: e.target.value.split("\n").filter(Boolean) } }))} placeholder="Un critère par ligne" value={definitionOfDone} />
            <button className="rounded-lg border px-4 py-2 text-sm font-bold disabled:opacity-40" disabled={locked || !!busy} onClick={() => void run("github", () => api.saveBuildGithub(phase.id, data.github, token))}>Enregistrer la préparation</button>
          </div>
        </article>
        <article className="rounded-2xl border border-black/10 p-5 lg:col-span-2">
          <h3 className="font-semibold">Décisions techniques d’exécution</h3>
          <form className="mt-4 grid gap-2 sm:grid-cols-2" onSubmit={(e) => void addDecision(e)}>
            <input className="rounded-lg border p-2 text-sm" disabled={locked} onChange={(e) => setDecisionTitle(e.target.value)} placeholder="Titre de la décision" required value={decisionTitle} />
            <input className="rounded-lg border p-2 text-sm" disabled={locked} onChange={(e) => setDecisionText(e.target.value)} placeholder="Décision retenue" required value={decisionText} />
            <button className="rounded-lg bg-black px-4 py-2 text-sm font-bold text-white disabled:opacity-40 sm:col-span-2" disabled={locked || !!busy}>Consigner comme acceptée</button>
          </form>
          <div className="mt-4 flex flex-wrap gap-2">{data.decisions.map((decision) => <span className="rounded-full border px-3 py-1 text-xs" key={decision.id}>{decision.code} · {decision.title} · {decision.status}</span>)}</div>
        </article>
      </div>
      {error && <p className="mt-4 rounded-xl bg-red-50 p-3 text-sm text-red-700">{error}</p>}
      <div className="mt-6 flex flex-wrap gap-3">
        <button className="rounded-full bg-[#d9503f] px-5 py-2 text-sm font-bold text-white disabled:opacity-35" disabled={locked || !!busy} onClick={() => void (async () => { setBusy("plan"); setError(""); try { await api.generateBuildPlan(phase.id, token); setData(await api.build(phase.id, token)); await onChanged(); } catch { setError("Complétez le Design Pack, le backlog, les liens, une décision acceptée et GitHub."); } finally { setBusy(""); } })()}>{data.artifacts.includes("BUILD_PLAN") ? "✓ Régénérer le Build Plan" : "Générer le Build Plan"}</button>
        <button className="rounded-full border border-black px-5 py-2 text-sm font-bold disabled:opacity-35" disabled={locked || !!busy || !data.artifacts.includes("BUILD_PLAN")} onClick={() => void (async () => { setBusy("validate"); try { await api.validatePhase(phase.id, token, "Build Plan prêt pour la phase Tester"); await onChanged(); } catch { setError("Le Build Plan doit être prêt."); } finally { setBusy(""); } })()}>{phase.status === "VALIDATED" ? "Phase validée" : "Valider Coder →"}</button>
      </div>
    </section>
  );
}
