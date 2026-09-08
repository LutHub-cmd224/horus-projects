import { useEffect, useState } from "react";
import { api, type ModelWorkspaceData, type OverviewPhase } from "./api";

const empty: ModelWorkspaceData = {
  entities: [],
  relationships: [],
  business_rules: [],
  artifacts: [],
};
const cardinalities = ["0..1", "1", "0..N", "1..N"];

export function ModelWorkspace({
  phase,
  token,
  onChanged,
}: {
  phase: OverviewPhase;
  token: string;
  onChanged: () => Promise<void>;
}) {
  const [model, setModel] = useState<ModelWorkspaceData>(empty);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let cancelled = false;
    api
      .model(phase.id, token)
      .then((v) => {
        if (!cancelled) setModel(v);
      })
      .catch(() => {
        if (!cancelled) setError("Impossible de charger le modèle.");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [phase.id, token]);
  function updateEntity(
    index: number,
    key: "conceptual_name" | "logical_name" | "physical_name",
    value: string,
  ) {
    setModel((v) => ({
      ...v,
      entities: v.entities.map((e, i) =>
        i === index ? { ...e, [key]: value } : e,
      ),
    }));
  }
  function addEntity() {
    setModel((v) => ({
      ...v,
      entities: [
        ...v.entities,
        {
          conceptual_name: `Entité ${v.entities.length + 1}`,
          logical_name: "",
          physical_name: "",
          description: null,
          attributes: [],
        },
      ],
    }));
  }
  function addAttribute(index: number) {
    setModel((v) => ({
      ...v,
      entities: v.entities.map((e, i) =>
        i === index
          ? {
              ...e,
              attributes: [
                ...e.attributes,
                {
                  conceptual_name: `Attribut ${e.attributes.length + 1}`,
                  logical_name: "",
                  physical_name: "",
                  data_type: "",
                  is_primary_key: e.attributes.length === 0,
                  is_unique: false,
                  is_nullable: e.attributes.length !== 0,
                  default_value: null,
                },
              ],
            }
          : e,
      ),
    }));
  }
  function updateAttribute(
    ei: number,
    ai: number,
    key: "conceptual_name" | "logical_name" | "physical_name" | "data_type",
    value: string,
  ) {
    setModel((v) => ({
      ...v,
      entities: v.entities.map((e, i) =>
        i === ei
          ? {
              ...e,
              attributes: e.attributes.map((a, j) =>
                j === ai ? { ...a, [key]: value } : a,
              ),
            }
          : e,
      ),
    }));
  }
  async function save() {
    setBusy("save");
    setError("");
    try {
      setModel(await api.saveModel(phase.id, model, token));
      await onChanged();
    } catch {
      setError(
        "Enregistrement impossible. Vérifiez les noms et les relations.",
      );
    } finally {
      setBusy("");
    }
  }
  async function generate(level: string) {
    setBusy(level);
    setError("");
    try {
      await api.generateModelArtifact(phase.id, level, token);
      setModel(await api.model(phase.id, token));
      await onChanged();
    } catch {
      setError(`${level} incomplet : vérifiez les informations requises.`);
    } finally {
      setBusy("");
    }
  }
  if (loading)
    return (
      <div className="mt-14 rounded-[2rem] border border-black/10 p-10 text-sm text-black/45">
        Chargement du workspace Modéliser…
      </div>
    );
  const locked = phase.status === "VALIDATED";
  return (
    <section
      className="mt-14 rounded-[2rem] border border-black/10 bg-white/55 p-6 sm:p-8"
      id="model-workspace"
    >
      <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#d9503f]">
        Phase 02 · Modéliser
      </p>
      <div className="mt-3 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h2 className="text-4xl font-semibold tracking-[-0.05em]">
            Du métier à la base de données.
          </h2>
          <p className="mt-3 text-sm text-black/50">
            Structurez le MCD, traduisez-le en MLD puis précisez le MPD.
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
      <div className="mt-8 grid gap-5 xl:grid-cols-[1.3fr_.7fr]">
        <div className="space-y-4">
          <div className="flex justify-between">
            <h3 className="font-semibold">Entités et attributs</h3>
            <button
              className="text-sm font-semibold text-[#d9503f]"
              disabled={locked}
              onClick={addEntity}
            >
              + Entité
            </button>
          </div>
          {model.entities.map((e, ei) => (
            <article
              className="rounded-2xl border border-black/10 bg-white/70 p-4"
              key={ei}
            >
              <div className="grid gap-2 sm:grid-cols-3">
                {(
                  ["conceptual_name", "logical_name", "physical_name"] as const
                ).map((key) => (
                  <input
                    className="rounded-xl border border-black/10 px-3 py-2 text-sm"
                    disabled={locked}
                    key={key}
                    onChange={(x) => updateEntity(ei, key, x.target.value)}
                    placeholder={
                      key === "conceptual_name"
                        ? "MCD : entité"
                        : key === "logical_name"
                          ? "MLD : relation"
                          : "MPD : table"
                    }
                    value={e[key] ?? ""}
                  />
                ))}
              </div>
              <div className="mt-3 space-y-2">
                {e.attributes.map((a, ai) => (
                  <div className="grid gap-2 sm:grid-cols-4" key={ai}>
                    {(
                      [
                        "conceptual_name",
                        "logical_name",
                        "physical_name",
                        "data_type",
                      ] as const
                    ).map((key) => (
                      <input
                        className="rounded-lg border border-black/10 px-3 py-2 text-xs"
                        disabled={locked}
                        key={key}
                        onChange={(x) =>
                          updateAttribute(ei, ai, key, x.target.value)
                        }
                        placeholder={key.replace("_name", "")}
                        value={a[key] ?? ""}
                      />
                    ))}
                  </div>
                ))}
              </div>
              <button
                className="mt-3 text-xs font-semibold"
                disabled={locked}
                onClick={() => addAttribute(ei)}
              >
                + Attribut
              </button>
            </article>
          ))}
        </div>
        <aside className="space-y-5">
          <article className="rounded-2xl bg-[#1b1b18] p-5 text-white">
            <div className="flex justify-between">
              <h3 className="font-semibold">Relations</h3>
              <button
                disabled={locked || model.entities.length < 2}
                onClick={() =>
                  setModel((v) => ({
                    ...v,
                    relationships: [
                      ...v.relationships,
                      {
                        name: "Relation",
                        source_entity: v.entities[0].conceptual_name,
                        target_entity: v.entities[1].conceptual_name,
                        source_cardinality: "1",
                        target_cardinality: "0..N",
                        description: null,
                      },
                    ],
                  }))
                }
              >
                +
              </button>
            </div>
            {model.relationships.map((r, i) => (
              <div className="mt-3 rounded-xl bg-white/10 p-3 text-xs" key={i}>
                {r.name} · {r.source_entity} ({r.source_cardinality}) →{" "}
                {r.target_entity} ({r.target_cardinality})
                <select
                  className="mt-2 bg-transparent"
                  disabled={locked}
                  onChange={(e) =>
                    setModel((v) => ({
                      ...v,
                      relationships: v.relationships.map((x, j) =>
                        j === i
                          ? { ...x, target_cardinality: e.target.value }
                          : x,
                      ),
                    }))
                  }
                  value={r.target_cardinality}
                >
                  {cardinalities.map((c) => (
                    <option className="text-black" key={c}>
                      {c}
                    </option>
                  ))}
                </select>
              </div>
            ))}
          </article>
          <article className="rounded-2xl border border-black/10 p-5">
            <div className="flex justify-between">
              <h3 className="font-semibold">Règles métier</h3>
              <button
                disabled={locked}
                onClick={() =>
                  setModel((v) => ({
                    ...v,
                    business_rules: [
                      ...v.business_rules,
                      { title: "Nouvelle règle", description: "À préciser" },
                    ],
                  }))
                }
              >
                +
              </button>
            </div>
            {model.business_rules.map((r, i) => (
              <div className="mt-3">
                <input
                  className="w-full border-b border-black/10 bg-transparent py-1 text-sm font-semibold"
                  disabled={locked}
                  onChange={(e) =>
                    setModel((v) => ({
                      ...v,
                      business_rules: v.business_rules.map((x, j) =>
                        j === i ? { ...x, title: e.target.value } : x,
                      ),
                    }))
                  }
                  value={r.title}
                />
                <textarea
                  className="mt-1 w-full bg-transparent text-xs text-black/55"
                  disabled={locked}
                  onChange={(e) =>
                    setModel((v) => ({
                      ...v,
                      business_rules: v.business_rules.map((x, j) =>
                        j === i ? { ...x, description: e.target.value } : x,
                      ),
                    }))
                  }
                  value={r.description}
                />
              </div>
            ))}
          </article>
        </aside>
      </div>
      {error && (
        <p className="mt-5 rounded-xl bg-red-50 p-3 text-sm text-red-700">
          {error}
        </p>
      )}
      <div className="mt-6 flex flex-wrap gap-3">
        {["MCD", "MLD", "MPD"].map((level) => (
          <button
            className={`rounded-full border px-5 py-2 text-sm font-semibold ${model.artifacts.includes(level) ? "border-black bg-black text-white" : "border-black/15"}`}
            disabled={locked || !!busy}
            key={level}
            onClick={() => void generate(level)}
          >
            {model.artifacts.includes(level) ? "✓ " : ""}Générer {level}
          </button>
        ))}
      </div>
    </section>
  );
}
