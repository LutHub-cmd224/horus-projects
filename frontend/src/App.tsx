const phases = ['Analyser', 'Modéliser', 'Concevoir', 'Coder', 'Tester', 'Déployer'];

export default function App() {
  return (
    <main className="min-h-screen bg-stone-50 px-6 py-10 text-stone-950 sm:px-10">
      <section className="mx-auto flex min-h-[calc(100vh-5rem)] max-w-6xl flex-col justify-center">
        <p className="text-xs font-bold tracking-[0.2em] text-stone-500">HORUS PROJECTS</p>
        <h1 className="mt-6 max-w-5xl text-6xl font-semibold leading-[0.9] tracking-[-0.06em] sm:text-8xl lg:text-9xl">
          Voir avant de construire.
        </h1>
        <p className="mt-8 max-w-2xl text-lg leading-8 text-stone-600 sm:text-xl">
          Analysez, modélisez et concevez votre produit avant de passer au code.
        </p>

        <div className="mt-14 grid grid-cols-2 overflow-hidden border border-stone-300 sm:grid-cols-3">
          {phases.map((phase, index) => (
            <div
              className="flex min-h-32 flex-col justify-between border-b border-r border-stone-300 p-5 last:border-b-0"
              key={phase}
            >
              <span className="text-xs text-stone-500">{String(index + 1).padStart(2, '0')}</span>
              <strong className="text-sm font-semibold sm:text-base">{phase}</strong>
            </div>
          ))}
        </div>
      </section>
    </main>
  );
}
