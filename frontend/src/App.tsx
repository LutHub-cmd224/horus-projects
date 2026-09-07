const phases = [
  { number: '01', name: 'Analyser', status: 'Validée', progress: 100 },
  { number: '02', name: 'Modéliser', status: 'Validée', progress: 100 },
  { number: '03', name: 'Concevoir', status: 'En cours', progress: 68 },
  { number: '04', name: 'Coder', status: 'À venir', progress: 0 },
  { number: '05', name: 'Tester', status: 'À venir', progress: 0 },
  { number: '06', name: 'Déployer', status: 'À venir', progress: 0 },
];

const metrics = [
  ['12', 'Exigences'],
  ['08', 'Tâches ouvertes'],
  ['05', 'Décisions'],
];

export default function App() {
  return (
    <main className="min-h-screen bg-[#f3f0e9] text-[#171714]">
      <div className="mx-auto grid min-h-screen max-w-[1600px] lg:grid-cols-[240px_1fr]">
        <aside className="hidden border-r border-black/10 px-7 py-8 lg:flex lg:flex-col">
          <div>
            <div className="flex items-center gap-3">
              <div className="grid size-9 place-items-center rounded-full border border-black/20 text-[10px] font-bold tracking-widest">H</div>
              <div>
                <p className="text-sm font-semibold tracking-tight">HORUS</p>
                <p className="text-[10px] uppercase tracking-[0.2em] text-black/40">Projects</p>
              </div>
            </div>
            <nav className="mt-14 space-y-1 text-sm">
              <a className="block rounded-xl bg-black px-4 py-3 font-medium text-white" href="#dashboard">Vue d’ensemble</a>
              <a className="block rounded-xl px-4 py-3 text-black/55 transition hover:bg-black/5 hover:text-black" href="#phases">Phases</a>
              <a className="block rounded-xl px-4 py-3 text-black/55 transition hover:bg-black/5 hover:text-black" href="#activity">Activité</a>
            </nav>
          </div>
          <div className="mt-auto border-t border-black/10 pt-6 text-xs leading-5 text-black/45">
            HORUS Method v1.0<br />Voir avant d’agir.
          </div>
        </aside>

        <section id="dashboard" className="px-5 py-6 sm:px-8 lg:px-12 lg:py-9">
          <header className="flex items-center justify-between border-b border-black/10 pb-6">
            <div className="flex items-center gap-3 lg:hidden">
              <div className="grid size-9 place-items-center rounded-full border border-black/20 text-[10px] font-bold">H</div>
              <span className="text-sm font-semibold">HORUS</span>
            </div>
            <div className="hidden lg:block">
              <p className="text-xs uppercase tracking-[0.18em] text-black/40">Workspace personnel</p>
            </div>
            <button className="rounded-full border border-black/15 bg-white/40 px-4 py-2 text-xs font-semibold">Luther.cmd</button>
          </header>

          <div className="pt-10">
            <div className="flex flex-col gap-8 xl:flex-row xl:items-end xl:justify-between">
              <div>
                <p className="text-xs font-semibold uppercase tracking-[0.2em] text-black/45">Projet actif · SaaS</p>
                <h1 className="mt-4 text-5xl font-semibold tracking-[-0.055em] sm:text-6xl lg:text-7xl">HORUS Projects</h1>
                <p className="mt-5 max-w-2xl text-base leading-7 text-black/50 sm:text-lg">
                  Transformer une idée en produit structuré, sans brûler les étapes qui rendent un projet solide.
                </p>
              </div>
              <div className="min-w-64 rounded-2xl bg-[#d9503f] p-5 text-white shadow-[0_20px_60px_rgba(217,80,63,0.18)]">
                <div className="flex items-start justify-between">
                  <span className="text-xs uppercase tracking-[0.18em] text-white/65">Progression</span>
                  <span className="text-3xl font-semibold tracking-tight">45%</span>
                </div>
                <div className="mt-8 h-1 overflow-hidden rounded-full bg-white/25"><div className="h-full w-[45%] bg-white" /></div>
                <p className="mt-3 text-xs text-white/70">Phase 03 sur 06</p>
              </div>
            </div>

            <div className="mt-12 grid gap-4 md:grid-cols-3">
              {metrics.map(([value, label]) => (
                <article className="rounded-2xl border border-black/10 bg-white/45 p-5" key={label}>
                  <p className="text-3xl font-semibold tracking-[-0.04em]">{value}</p>
                  <p className="mt-2 text-xs uppercase tracking-[0.16em] text-black/40">{label}</p>
                </article>
              ))}
            </div>

            <section id="phases" className="mt-14">
              <div className="mb-5 flex items-end justify-between">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.18em] text-black/40">Méthode HORUS</p>
                  <h2 className="mt-2 text-2xl font-semibold tracking-[-0.035em]">Les six passages obligés</h2>
                </div>
                <span className="hidden text-xs text-black/40 sm:block">2 validées · 1 en cours</span>
              </div>
              <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                {phases.map((phase) => {
                  const active = phase.status === 'En cours';
                  const complete = phase.status === 'Validée';
                  return (
                    <article
                      className={`group min-h-48 rounded-2xl border p-5 transition ${active ? 'border-[#d9503f] bg-[#d9503f] text-white shadow-[0_18px_50px_rgba(217,80,63,0.16)]' : 'border-black/10 bg-white/45 hover:-translate-y-0.5 hover:bg-white/70'}`}
                      key={phase.name}
                    >
                      <div className="flex items-start justify-between">
                        <span className={`text-xs ${active ? 'text-white/60' : 'text-black/35'}`}>{phase.number}</span>
                        <span className={`rounded-full px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider ${active ? 'bg-white/15 text-white' : complete ? 'bg-black text-white' : 'border border-black/10 text-black/35'}`}>{phase.status}</span>
                      </div>
                      <div className="mt-16 flex items-end justify-between gap-4">
                        <div>
                          <h3 className="text-xl font-semibold tracking-[-0.03em]">{phase.name}</h3>
                          <p className={`mt-1 text-xs ${active ? 'text-white/65' : 'text-black/40'}`}>{phase.progress}% complété</p>
                        </div>
                        <span className={`text-xl ${active ? 'text-white' : 'text-black/25'}`}>↗</span>
                      </div>
                    </article>
                  );
                })}
              </div>
            </section>

            <section id="activity" className="mt-14 grid gap-4 xl:grid-cols-[1.4fr_0.6fr]">
              <article className="rounded-2xl border border-black/10 bg-[#1b1b18] p-6 text-white">
                <p className="text-xs uppercase tracking-[0.18em] text-white/40">Prochaine action</p>
                <div className="mt-8 flex flex-col gap-6 sm:flex-row sm:items-end sm:justify-between">
                  <div>
                    <p className="text-2xl font-medium tracking-[-0.035em]">Finaliser l’architecture technique</p>
                    <p className="mt-2 max-w-xl text-sm leading-6 text-white/45">Valider les derniers critères de la phase Concevoir avant de débloquer Coder.</p>
                  </div>
                  <button className="shrink-0 rounded-full bg-white px-5 py-3 text-xs font-bold text-black">Ouvrir la phase →</button>
                </div>
              </article>
              <article className="rounded-2xl border border-black/10 bg-white/45 p-6">
                <p className="text-xs uppercase tracking-[0.18em] text-black/40">Dernière décision</p>
                <p className="mt-8 text-lg font-semibold tracking-tight">DEC-005 · Modular Monolith</p>
                <p className="mt-2 text-sm leading-6 text-black/45">Acceptée · Architecture backend</p>
              </article>
            </section>
          </div>
        </section>
      </div>
    </main>
  );
}
