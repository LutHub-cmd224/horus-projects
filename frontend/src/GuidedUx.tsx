import type { ReactNode } from "react";

export function GuidedQuestion({
  number,
  title,
  help,
  example,
  children,
  complete = false,
}: {
  number: number;
  title: string;
  help: string;
  example?: string;
  children: ReactNode;
  complete?: boolean;
}) {
  return (
    <article className="rounded-3xl border border-black/10 bg-white/75 p-5 sm:p-7">
      <div className="flex gap-4">
        <span className={`grid size-9 shrink-0 place-items-center rounded-full text-sm font-bold ${complete ? "bg-emerald-600 text-white" : "bg-black text-white"}`}>
          {complete ? "✓" : number}
        </span>
        <div className="min-w-0 flex-1">
          <h3 className="text-xl font-semibold tracking-tight">{title}</h3>
          <p className="mt-1 text-sm text-black/55">{help}</p>
          {example && <p className="mt-2 text-xs text-black/40">Exemple : {example}</p>}
          <div className="mt-5">{children}</div>
        </div>
      </div>
    </article>
  );
}

export function Suggestions({ options, selected, onToggle }: { options: string[]; selected: string[]; onToggle: (value: string) => void }) {
  return <div className="flex flex-wrap gap-2">{options.map((option) => <button className={`rounded-full border px-4 py-2 text-sm ${selected.includes(option) ? "border-black bg-black text-white" : "border-black/15 bg-white"}`} key={option} onClick={() => onToggle(option)} type="button">{option}</button>)}</div>;
}

export function AdvancedSection({ children }: { children: ReactNode }) {
  return <details className="rounded-2xl border border-black/10 bg-black/[0.02] p-4"><summary className="cursor-pointer text-sm font-semibold">Voir les détails techniques</summary><div className="mt-4">{children}</div></details>;
}

export function NextAction({ children }: { children: ReactNode }) {
  return <p className="rounded-2xl bg-[#fff1e8] px-4 py-3 text-sm text-[#8b351f]"><strong>Prochaine étape :</strong> {children}</p>;
}

export function JourneyProgress({ current, total, label }: { current: number; total: number; label: string }) {
  return <div aria-label={`${current} étapes sur ${total}`} className="flex items-center justify-between gap-5"><div><p className="text-xs uppercase tracking-[.18em] text-black/35">{label}</p><p className="mt-1 text-sm font-semibold">{current} étape{current > 1 ? "s" : ""} sur {total}</p></div><div className="flex gap-2">{Array.from({length:total},(_,index)=><span className={`size-2 rounded-full transition ${index<current?"bg-[#d9503f]":"bg-black/10"}`} key={index}/>)}</div></div>;
}

export function SuggestionCard({ children, selected, onClick }: { children: ReactNode; selected?: boolean; onClick: () => void }) {
  return <button className={`w-full rounded-2xl border p-4 text-left text-sm leading-6 transition hover:-translate-y-0.5 ${selected?"border-black bg-black text-white":"border-black/10 bg-white/70 hover:border-black/25"}`} onClick={onClick} type="button">{children}</button>;
}

export function GeneratedProposal({ children, onUse }: { children: ReactNode; onUse?: () => void }) {
  return <div className="rounded-2xl bg-[#1b1b18] p-5 text-white"><p className="text-[10px] uppercase tracking-[.18em] text-white/40">Proposition HORUS</p><div className="mt-3 text-sm leading-6 text-white/80">{children}</div>{onUse&&<button className="mt-4 rounded-full bg-white px-4 py-2 text-xs font-bold text-black" onClick={onUse} type="button">Utiliser cette formulation</button>}</div>;
}
