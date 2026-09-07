import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { AnalyzeWorkspace } from './AnalyzeWorkspace';
import { api, type Project, type ProjectOverview, type User, type Workspace } from './api';

const phaseNames: Record<string, string> = {
  ANALYZE: 'Analyser',
  MODEL: 'Modéliser',
  DESIGN: 'Concevoir',
  BUILD: 'Coder',
  TEST: 'Tester',
  DEPLOY: 'Déployer',
};

function phaseProgress(phase: ProjectOverview['phases'][number]) {
  if (phase.status === 'VALIDATED') return 100;
  if (phase.status === 'LOCKED') return 0;
  if (phase.required_criteria > 0) return Math.round((phase.completed_required_criteria / phase.required_criteria) * 100);
  return phase.status === 'IN_PROGRESS' ? 25 : 0;
}

function statusLabel(status: string) {
  if (status === 'VALIDATED') return 'Validée';
  if (status === 'IN_PROGRESS') return 'En cours';
  if (status === 'AVAILABLE') return 'Disponible';
  return 'À venir';
}

function AuthScreen({ onAuthenticated }: { onAuthenticated: (token: string) => void }) {
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError('');
    try {
      const tokens = mode === 'login'
        ? await api.login(email, password)
        : await api.register(email, password, displayName);
      sessionStorage.setItem('horus_access_token', tokens.access_token);
      sessionStorage.setItem('horus_refresh_token', tokens.refresh_token);
      onAuthenticated(tokens.access_token);
    } catch {
      setError(mode === 'login' ? 'Connexion impossible. Vérifiez vos identifiants.' : 'Inscription impossible. Utilisez un mot de passe de 12 caractères minimum.');
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="min-h-screen bg-[#f3f0e9] px-5 py-6 text-[#171714] sm:px-8">
      <div className="mx-auto grid min-h-[calc(100vh-3rem)] max-w-7xl overflow-hidden rounded-[2rem] border border-black/10 bg-white/35 lg:grid-cols-[1.15fr_0.85fr]">
        <section className="flex flex-col justify-between p-8 sm:p-12 lg:p-16">
          <div className="flex items-center gap-3"><div className="grid size-10 place-items-center rounded-full border border-black/20 text-xs font-bold">H</div><div><p className="text-sm font-semibold">HORUS</p><p className="text-[10px] uppercase tracking-[0.2em] text-black/40">Projects</p></div></div>
          <div className="my-20 max-w-3xl"><p className="text-xs font-semibold uppercase tracking-[0.22em] text-[#d9503f]">Voir avant d’agir</p><h1 className="mt-5 text-6xl font-semibold leading-[0.9] tracking-[-0.065em] sm:text-7xl xl:text-8xl">Construire devient la dernière étape.</h1><p className="mt-8 max-w-xl text-lg leading-8 text-black/50">Analysez, modélisez et concevez avant de coder. Chaque étape devient visible, traçable et actionnable.</p></div>
        </section>
        <section className="flex items-center bg-[#1b1b18] p-8 text-white sm:p-12 lg:p-16">
          <form className="w-full" onSubmit={submit}>
            <p className="text-xs uppercase tracking-[0.2em] text-white/40">{mode === 'login' ? 'Connexion' : 'Créer un compte'}</p>
            <h2 className="mt-3 text-3xl font-semibold tracking-[-0.04em]">{mode === 'login' ? 'Reprendre votre projet.' : 'Commencer avec HORUS.'}</h2>
            <div className="mt-10 space-y-4">{mode === 'register' && <input className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3" onChange={(e) => setDisplayName(e.target.value)} placeholder="Nom affiché" value={displayName} />}<input className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3" onChange={(e) => setEmail(e.target.value)} placeholder="Email" required type="email" value={email} /><input className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3" minLength={12} onChange={(e) => setPassword(e.target.value)} placeholder="Mot de passe" required type="password" value={password} /></div>
            {error && <p className="mt-4 text-sm text-red-300">{error}</p>}
            <button className="mt-6 w-full rounded-xl bg-[#d9503f] px-5 py-3.5 text-sm font-bold" disabled={busy}>{busy ? 'Chargement…' : mode === 'login' ? 'Se connecter' : 'Créer mon espace'}</button>
            <button className="mt-5 text-sm text-white/50" onClick={() => setMode(mode === 'login' ? 'register' : 'login')} type="button">{mode === 'login' ? 'Pas encore de compte ? S’inscrire' : 'Déjà un compte ? Se connecter'}</button>
          </form>
        </section>
      </div>
    </main>
  );
}

function EmptyProject({ token, workspaces, onCreated }: { token: string; workspaces: Workspace[]; onCreated: (project: Project) => void }) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [busy, setBusy] = useState(false);
  async function create(event: FormEvent) {
    event.preventDefault();
    if (!workspaces[0]) return;
    setBusy(true);
    try { onCreated(await api.createProject(workspaces[0].id, name, description, token)); } finally { setBusy(false); }
  }
  return <div className="grid min-h-screen place-items-center bg-[#f3f0e9] p-6"><form className="w-full max-w-xl rounded-[2rem] border border-black/10 bg-white/50 p-10" onSubmit={create}><p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#d9503f]">Premier projet</p><h1 className="mt-4 text-4xl font-semibold tracking-[-0.05em]">Donnez une forme à votre idée.</h1><p className="mt-4 text-sm text-black/50">HORUS crée les six phases et débloque Analyser.</p><input className="mt-8 w-full rounded-xl border border-black/10 bg-white/70 px-4 py-3" onChange={(e) => setName(e.target.value)} placeholder="Nom du projet" required value={name} /><textarea className="mt-3 min-h-28 w-full rounded-xl border border-black/10 bg-white/70 px-4 py-3" onChange={(e) => setDescription(e.target.value)} placeholder="En une phrase, qu’est-ce que vous construisez ?" value={description} /><button className="mt-5 rounded-full bg-black px-6 py-3 text-sm font-bold text-white" disabled={busy || !workspaces.length}>{busy ? 'Création…' : 'Créer le projet →'}</button></form></div>;
}

function Dashboard({ token, overview, user, projects, selectedProjectId, onSelectProject, onExit, onRefresh }: { token: string; overview: ProjectOverview; user: User | null; projects: Project[]; selectedProjectId: string; onSelectProject: (id: string) => void; onExit: () => void; onRefresh: () => Promise<void> }) {
  const phases = overview.phases.map((phase) => ({ ...phase, name: phaseNames[phase.phase_type] ?? phase.phase_type, progress: phaseProgress(phase), label: statusLabel(phase.status) }));
  const progress = Math.round(phases.reduce((sum, phase) => sum + phase.progress, 0) / Math.max(phases.length, 1));
  const current = phases.find((phase) => phase.status === 'IN_PROGRESS') ?? phases.find((phase) => phase.status === 'AVAILABLE') ?? phases.at(-1);
  const analyze = overview.phases.find((phase) => phase.phase_type === 'ANALYZE');
  const validated = phases.filter((phase) => phase.status === 'VALIDATED').length;

  return (
    <main className="min-h-screen bg-[#f3f0e9] text-[#171714]">
      <div className="mx-auto grid min-h-screen max-w-[1600px] lg:grid-cols-[240px_1fr]">
        <aside className="hidden border-r border-black/10 px-7 py-8 lg:flex lg:flex-col"><div><div className="flex items-center gap-3"><div className="grid size-9 place-items-center rounded-full border border-black/20 text-[10px] font-bold">H</div><div><p className="text-sm font-semibold">HORUS</p><p className="text-[10px] uppercase tracking-[0.2em] text-black/40">Projects</p></div></div><nav className="mt-14 space-y-1 text-sm"><a className="block rounded-xl bg-black px-4 py-3 text-white" href="#dashboard">Vue d’ensemble</a><a className="block rounded-xl px-4 py-3 text-black/55" href="#phases">Phases</a><a className="block rounded-xl px-4 py-3 text-black/55" href="#analyze-workspace">Analyser</a></nav></div><div className="mt-auto border-t border-black/10 pt-6 text-xs text-black/45">HORUS Method v1.0<br />Voir avant d’agir.</div></aside>
        <section className="px-5 py-6 sm:px-8 lg:px-12 lg:py-9" id="dashboard">
          <header className="flex items-center justify-between gap-4 border-b border-black/10 pb-6"><div className="flex items-center gap-3 lg:hidden"><div className="grid size-9 place-items-center rounded-full border border-black/20 text-[10px] font-bold">H</div><span className="text-sm font-semibold">HORUS</span></div><select className="max-w-64 rounded-full border border-black/10 bg-white/50 px-4 py-2 text-xs" onChange={(e) => onSelectProject(e.target.value)} value={selectedProjectId}>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select><button className="rounded-full border border-black/15 bg-white/40 px-4 py-2 text-xs font-semibold" onClick={onExit}>{user?.display_name ?? user?.email ?? 'Déconnexion'}</button></header>
          <div className="pt-10">
            <div className="flex flex-col gap-8 xl:flex-row xl:items-end xl:justify-between"><div><p className="text-xs font-semibold uppercase tracking-[0.2em] text-black/45">Projet · {overview.project.status}</p><h1 className="mt-4 text-5xl font-semibold tracking-[-0.055em] sm:text-6xl lg:text-7xl">{overview.project.name}</h1><p className="mt-5 max-w-2xl text-lg leading-7 text-black/50">{overview.project.description ?? 'Structurez le projet phase par phase avec la méthode HORUS.'}</p></div><div className="min-w-64 rounded-2xl bg-[#d9503f] p-5 text-white"><div className="flex justify-between"><span className="text-xs uppercase tracking-[0.18em] text-white/65">Progression</span><span className="text-3xl font-semibold">{progress}%</span></div><div className="mt-8 h-1 bg-white/25"><div className="h-full bg-white" style={{ width: `${progress}%` }} /></div><p className="mt-3 text-xs text-white/70">Phase {String(current?.position ?? 1).padStart(2, '0')} sur 06</p></div></div>
            <section className="mt-14" id="phases"><div className="mb-5 flex items-end justify-between"><div><p className="text-xs uppercase tracking-[0.18em] text-black/40">Méthode HORUS</p><h2 className="mt-2 text-2xl font-semibold">Les six passages obligés</h2></div><span className="hidden text-xs text-black/40 sm:block">{validated} validée{validated > 1 ? 's' : ''} · {current?.name}</span></div><div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">{phases.map((phase) => { const active = phase.status === 'IN_PROGRESS' || phase.status === 'AVAILABLE'; const complete = phase.status === 'VALIDATED'; return <article className={`min-h-48 rounded-2xl border p-5 ${active ? 'border-[#d9503f] bg-[#d9503f] text-white' : 'border-black/10 bg-white/45'}`} key={phase.id}><div className="flex justify-between"><span className={active ? 'text-xs text-white/60' : 'text-xs text-black/35'}>{String(phase.position).padStart(2, '0')}</span><span className={`rounded-full px-2.5 py-1 text-[10px] font-semibold uppercase ${active ? 'bg-white/15' : complete ? 'bg-black text-white' : 'border border-black/10 text-black/35'}`}>{phase.label}</span></div><div className="mt-16"><h3 className="text-xl font-semibold">{phase.name}</h3><p className={active ? 'mt-1 text-xs text-white/65' : 'mt-1 text-xs text-black/40'}>{phase.progress}% complété</p></div></article>; })}</div></section>
            {analyze && <AnalyzeWorkspace onChanged={onRefresh} phase={analyze} token={token} />}
          </div>
        </section>
      </div>
    </main>
  );
}

export default function AppV2() {
  const [token, setToken] = useState(() => sessionStorage.getItem('horus_access_token') ?? '');
  const [user, setUser] = useState<User | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState('');
  const [overview, setOverview] = useState<ProjectOverview | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!token) return;
    let cancelled = false;
    setLoading(true);
    Promise.all([api.me(token), api.projects(token), api.workspaces(token)])
      .then(([nextUser, nextProjects, nextWorkspaces]) => {
        if (cancelled) return;
        setUser(nextUser);
        setProjects(nextProjects);
        setWorkspaces(nextWorkspaces);
        setSelectedProjectId((current) => current || nextProjects[0]?.id || '');
      })
      .catch(() => {
        if (cancelled) return;
        sessionStorage.clear();
        setToken('');
      })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [token]);

  async function refreshOverview() {
    if (!token || !selectedProjectId) return;
    setOverview(await api.overview(selectedProjectId, token));
  }

  useEffect(() => {
    if (!token || !selectedProjectId) return;
    void refreshOverview();
  }, [selectedProjectId, token]);

  const selectedExists = useMemo(() => projects.some((project) => project.id === selectedProjectId), [projects, selectedProjectId]);

  function exit() {
    sessionStorage.clear();
    setToken('');
    setUser(null);
    setProjects([]);
    setOverview(null);
  }

  if (!token) return <AuthScreen onAuthenticated={setToken} />;
  if (loading && !projects.length) return <div className="grid min-h-screen place-items-center bg-[#f3f0e9] text-sm text-black/45">HORUS prépare votre espace…</div>;
  if (!projects.length) return <EmptyProject onCreated={(project) => { setProjects([project]); setSelectedProjectId(project.id); }} token={token} workspaces={workspaces} />;
  if (!overview || !selectedExists) return <div className="grid min-h-screen place-items-center bg-[#f3f0e9] text-sm text-black/45">Chargement du projet…</div>;
  return <Dashboard onExit={exit} onRefresh={refreshOverview} onSelectProject={setSelectedProjectId} overview={overview} projects={projects} selectedProjectId={selectedProjectId} token={token} user={user} />;
}
