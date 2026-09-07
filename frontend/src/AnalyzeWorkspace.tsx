import { useEffect, useMemo, useState } from 'react';
import { api, type OverviewPhase, type ValidationCriterion } from './api';

type AnalyzeWorkspaceProps = {
  phase: OverviewPhase;
  token: string;
  onChanged: () => Promise<void> | void;
};

export function AnalyzeWorkspace({ phase, token, onChanged }: AnalyzeWorkspaceProps) {
  const [criteria, setCriteria] = useState<ValidationCriterion[]>([]);
  const [busyId, setBusyId] = useState('');
  const [validating, setValidating] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    let cancelled = false;
    api.criteria(phase.id, token)
      .then((nextCriteria) => {
        if (!cancelled) setCriteria(nextCriteria);
      })
      .catch(() => {
        if (!cancelled) setError('Impossible de charger les critères de cette phase.');
      });
    return () => { cancelled = true; };
  }, [phase.id, token]);

  const required = useMemo(() => criteria.filter((criterion) => criterion.required), [criteria]);
  const completed = required.filter((criterion) => criterion.completed).length;
  const canValidate = required.length > 0 && completed === required.length && phase.status !== 'VALIDATED';

  async function ensureStarted() {
    if (phase.status !== 'AVAILABLE') return;
    await api.startPhase(phase.id, token);
    await onChanged();
  }

  async function toggleCriterion(criterion: ValidationCriterion) {
    setBusyId(criterion.id);
    setError('');
    try {
      await ensureStarted();
      await api.updateCriterion(phase.id, criterion.id, !criterion.completed, token);
      setCriteria((current) => current.map((item) => item.id === criterion.id ? { ...item, completed: !item.completed } : item));
      await onChanged();
    } catch {
      setError('La mise à jour du critère a échoué.');
    } finally {
      setBusyId('');
    }
  }

  async function validate() {
    setValidating(true);
    setError('');
    try {
      await api.validatePhase(phase.id, token, 'Validation depuis HORUS Projects');
      await onChanged();
    } catch {
      setError('La phase ne peut pas encore être validée. Vérifiez les critères requis.');
    } finally {
      setValidating(false);
    }
  }

  return (
    <section className="mt-14 overflow-hidden rounded-[2rem] border border-black/10 bg-white/55" id="analyze-workspace">
      <div className="grid gap-8 p-6 sm:p-8 xl:grid-cols-[0.8fr_1.2fr]">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#d9503f]">Phase 01 · Analyser</p>
          <h2 className="mt-4 text-4xl font-semibold tracking-[-0.05em]">Voir clairement avant de construire.</h2>
          <p className="mt-5 max-w-lg text-sm leading-6 text-black/50">Formalisez le problème, la cible, la valeur, les objectifs, les contraintes et le périmètre MVP. HORUS bloque la suite tant que l’analyse minimale n’est pas terminée.</p>
          <div className="mt-8 rounded-2xl bg-[#1b1b18] p-5 text-white"><div className="flex items-end justify-between gap-4"><div><p className="text-xs uppercase tracking-[0.16em] text-white/45">Progression requise</p><p className="mt-2 text-sm text-white/60">{completed} / {required.length} critères</p></div><p className="text-4xl font-semibold">{required.length ? Math.round((completed / required.length) * 100) : 0}%</p></div><div className="mt-6 h-1 overflow-hidden bg-white/15"><div className="h-full bg-white transition-all" style={{ width: `${required.length ? (completed / required.length) * 100 : 0}%` }} /></div></div>
        </div>
        <div>
          {!criteria.length && !error ? <div className="grid min-h-64 place-items-center text-sm text-black/40">Chargement des critères…</div> : <div className="space-y-3">{criteria.map((criterion, index) => <button className={`flex w-full items-center gap-4 rounded-2xl border p-4 text-left transition ${criterion.completed ? 'border-black bg-black text-white' : 'border-black/10 bg-white/65 hover:border-black/25'} disabled:cursor-not-allowed disabled:opacity-60`} disabled={phase.status === 'VALIDATED' || busyId === criterion.id} key={criterion.id} onClick={() => void toggleCriterion(criterion)} type="button"><span className={`grid size-8 shrink-0 place-items-center rounded-full border text-xs font-semibold ${criterion.completed ? 'border-white/20 bg-white text-black' : 'border-black/15'}`}>{criterion.completed ? '✓' : String(index + 1).padStart(2, '0')}</span><span className="flex-1"><span className="block text-sm font-semibold">{criterion.label}</span><span className={`mt-1 block text-[11px] uppercase tracking-[0.14em] ${criterion.completed ? 'text-white/45' : 'text-black/35'}`}>{criterion.required ? 'Requis' : 'Optionnel'} · {criterion.code}</span></span></button>)}</div>}
          {error && <p className="mt-4 rounded-xl bg-red-50 px-4 py-3 text-sm text-red-700">{error}</p>}
          <div className="mt-6 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"><p className="text-xs text-black/40">{phase.status === 'VALIDATED' ? 'Analyse validée. Modéliser est maintenant disponible.' : canValidate ? 'Tous les critères requis sont terminés.' : 'Complétez les critères requis pour valider.'}</p><button className="rounded-full bg-[#d9503f] px-6 py-3 text-sm font-bold text-white disabled:cursor-not-allowed disabled:opacity-35" disabled={!canValidate || validating} onClick={() => void validate()} type="button">{validating ? 'Validation…' : phase.status === 'VALIDATED' ? 'Phase validée' : 'Valider Analyser →'}</button></div>
        </div>
      </div>
    </section>
  );
}
