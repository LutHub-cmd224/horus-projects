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
