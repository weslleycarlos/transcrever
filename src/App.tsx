import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle, ArrowLeft, BarChart3, Calendar, Check, Clock, Download, FileAudio,
  FolderOpen, HardDrive, Home, ListVideo, Loader, Pencil, Play,
  Plus, RefreshCw, Search, Settings, Trash2, X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { JobRow, ProfileRow, ScanResponse, TranscriptionView } from "./types";

type NavSection = "home" | "queue" | "review" | "settings";

const EMPTY_PROFILE: ProfileRow = {
  id: 0, name: "", backend: "faster_whisper", modelPath: "", device: "cpu",
  precision: "float16", threads: 4, language: "pt", task: "transcribe", advancedJson: "{}",
};

export default function App() {
  const [nav, setNav] = useState<NavSection>("home");
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
  const [message, setMessage] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const [jobs, setJobs] = useState<JobRow[]>([]);
  const [queuedCount, setQueuedCount] = useState(0);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const [profiles, setProfiles] = useState<ProfileRow[]>([]);
  const [activeProfile, setActiveProfile] = useState<ProfileRow | null>(null);
  const [selectedJob, setSelectedJob] = useState<JobRow | null>(null);
  const [transcription, setTranscription] = useState<TranscriptionView | null>(null);
  const canStart = queuedCount > 0 && activeProfile !== null && !isRunning;

  useEffect(() => { loadProfiles(); loadActiveProfile(); return () => stopPolling(); }, []);

  const startPolling = useCallback(() => {
    if (pollingRef.current) return;
    pollingRef.current = setInterval(async () => {
      try {
        const list = await invoke<JobRow[]>("list_jobs");
        setJobs(list);
        const pending = list.filter((j) => j.status === "pending" || j.status === "processing");
        if (pending.length === 0 && list.length > 0) { setIsRunning(false); stopPolling(); setMessage("Transcricao concluida."); }
      } catch { /* */ }
    }, 1500);
  }, []);
  const stopPolling = () => { if (pollingRef.current) { clearInterval(pollingRef.current); pollingRef.current = null; } };
  async function loadProfiles() { try { setProfiles(await invoke<ProfileRow[]>("list_profiles")); } catch { /* */ } }
  async function loadActiveProfile() { try { setActiveProfile(await invoke<ProfileRow | null>("get_active_profile")); } catch { /* */ } }
  async function handleScan() { if (!source) return; try { const r = await invoke<ScanResponse>("scan_source_folder", { path: source }); setQueuedCount(r.queuedCount); setMessage(r.discoveredCount + " arquivos encontrados, " + r.queuedCount + " na fila."); } catch (e) { setMessage("Erro: " + formatError(e)); } }
  async function handleStart() { if (!canStart) return; try { await invoke("start_transcription"); setIsRunning(true); setNav("queue"); setMessage("Transcricao em andamento..."); startPolling(); } catch (e) { setMessage("Erro: " + formatError(e)); } }
  async function handleViewJob(job: JobRow) { if (job.status !== "completed") return; setSelectedJob(job); try { setTranscription(await invoke<TranscriptionView | null>("get_transcription", { jobId: job.jobId })); } catch (e) { setMessage("Erro: " + formatError(e)); } }

  const stats = useMemo(() => {
    const t = jobs.length, p = jobs.filter(j => j.status === "pending").length, r = jobs.filter(j => j.status === "processing").length;
    const c = jobs.filter(j => j.status === "completed").length, e = jobs.filter(j => j.status === "error").length;
    return { total: t, pending: p, processing: r, completed: c, errors: e, progress: t > 0 ? ((c + e) / t) * 100 : 0 };
  }, [jobs]);

  return (
    <main className="app-shell">
      <nav className="sidebar">
        <h1 className="app-logo">Transcrever</h1>
        <NavItem icon={<Home size={18} />} label="Inicio" active={nav === "home"} onClick={() => setNav("home")} />
        <NavItem icon={<ListVideo size={18} />} label="Fila" active={nav === "queue"} onClick={() => setNav("queue")} badge={jobs.length || undefined} />
        <NavItem icon={<Pencil size={18} />} label="Revisao" active={nav === "review"} onClick={() => setNav("review")} badge={stats.completed || undefined} />
        <NavItem icon={<Settings size={18} />} label="Config" active={nav === "settings"} onClick={() => setNav("settings")} />
        <div className="sidebar-footer">{activeProfile && <div className="sidebar-profile-badge"><Check size={12} /> {activeProfile.name}</div>}</div>
      </nav>
      <section className="workspace">
        {nav === "home" && <HomeView source={source} setSource={setSource} destination={destination} setDestination={setDestination} message={message} setMessage={setMessage} queuedCount={queuedCount} setQueuedCount={setQueuedCount} isRunning={isRunning} canStart={canStart} onScan={handleScan} onStart={handleStart} />}
        {nav === "queue" && <QueueView stats={stats} jobs={jobs} message={message} isRunning={isRunning} canStart={canStart} onStart={handleStart} onViewJob={handleViewJob} />}
        {nav === "review" && <ReviewView jobs={jobs} selectedJob={selectedJob} transcription={transcription} onViewJob={handleViewJob} onBack={() => { setSelectedJob(null); setTranscription(null); }} />}
        {nav === "settings" && <SettingsView profiles={profiles} activeProfile={activeProfile} onProfilesChanged={() => { loadProfiles(); loadActiveProfile(); }} />}
      </section>
    </main>
  );
}

function NavItem({ icon, label, active, onClick, badge }: { icon: React.ReactNode; label: string; active: boolean; onClick: () => void; badge?: number }) {
  return (
    <button type="button" className={`nav-item ${active ? "nav-active" : ""}`} onClick={onClick}>
      {icon}<span>{label}</span>
      {badge !== undefined && badge > 0 && <span className="nav-badge">{badge}</span>}
    </button>
  );
}

function HomeView({ source, setSource, destination, setDestination, message, setMessage, queuedCount, setQueuedCount, isRunning, canStart, onScan, onStart }: any) {
  async function chooseSource() { const s = await open({ directory: true, multiple: false }); if (typeof s === "string") { setSource(s); setMessage("Pasta selecionada. Clique em Escanear."); } }
  async function chooseDest() { const s = await open({ directory: true, multiple: false }); if (typeof s === "string") { setDestination(s); try { await invoke("set_export_folder", { path: s }); } catch { } } }
  async function rescan() { if (!source) return; setMessage("Re-escaneando..."); try { const r = await invoke<ScanResponse>("scan_source_folder", { path: source }); setQueuedCount(r.queuedCount); setMessage(r.discoveredCount + " arquivos, " + r.queuedCount + " na fila."); } catch (e) { setMessage("Erro: " + formatError(e)); } }
  return (
    <div className="view home-view">
      <h2>Inicio</h2>
      <div className="card">
        <label className="field">Pasta de origem<div className="field-row"><input type="text" readOnly value={source} placeholder="Nenhuma pasta" /><button type="button" onClick={chooseSource}><FolderOpen size={16} /></button></div></label>
        <div className="card-actions"><button type="button" disabled={!source} onClick={onScan}><Search size={14} /> Escanear</button><button type="button" disabled={!source} onClick={rescan}><RefreshCw size={14} /> Re-escanear</button></div>
      </div>
      <div className="card"><label className="field">Pasta de destino<div className="field-row"><input type="text" readOnly value={destination} placeholder="Opcional" /><button type="button" onClick={chooseDest}><Download size={16} /></button></div></label></div>
      {message && <div className="queue-summary">{message}</div>}
      {queuedCount > 0 && <div className="card card-start"><div className="start-info"><FileAudio size={20} /><span><strong>{queuedCount}</strong> arquivos prontos</span></div><button type="button" className="btn-start" disabled={!canStart} onClick={onStart}><Play size={16} /> {isRunning ? "Processando..." : "Iniciar transcricao"}</button>{!canStart && <p className="hint">Configure um perfil em <strong>Config</strong>.</p>}</div>}
    </div>
  );
}

function QueueView({ stats, jobs, message, isRunning, canStart, onStart, onViewJob }: any) {
  return (
    <div className="view queue-view"><h2>Fila</h2>
      <div className="stats-grid">
        <StatCard icon={<ListVideo size={18} />} label="Total" value={stats.total} color="gray" />
        <StatCard icon={<Clock size={18} />} label="Pendentes" value={stats.pending} color="slate" />
        <StatCard icon={<Loader size={18} className={stats.processing > 0 ? "spin" : ""} />} label="Processando" value={stats.processing} color="blue" />
        <StatCard icon={<Check size={18} />} label="Concluidos" value={stats.completed} color="green" />
        <StatCard icon={<AlertCircle size={18} />} label="Erros" value={stats.errors} color="red" />
      </div>
      {stats.total > 0 && <div className="progress-bar-container"><div className="progress-bar" style={{ width: stats.progress + "%" }} /></div>}
      {message && <div className="queue-summary">{message}</div>}
      {!isRunning && stats.pending > 0 && <button type="button" className="btn-start" disabled={!canStart} onClick={onStart} style={{ marginBottom: 12 }}><Play size={14} /> Retomar</button>}
      <div className="job-list">{jobs.map((job: JobRow) => (<div key={job.jobId} className={"job-row job-" + job.status + (job.status === "completed" ? " job-clickable" : "")} onClick={() => onViewJob(job)} role={job.status === "completed" ? "button" : undefined} tabIndex={job.status === "completed" ? 0 : undefined} onKeyDown={(e) => { if (e.key === "Enter" && job.status === "completed") onViewJob(job); }}><div className="job-info"><span className="job-name">{job.fileName}</span><span className="job-path">{job.relativePath}</span></div><div className="job-status-area"><JobStatusBadge status={job.status} />{job.errorMessage && <span className="job-error" title={job.errorMessage}>{job.errorMessage}</span>}</div></div>))}</div>
      {jobs.length === 0 && <p className="empty-hint">Nenhum job. Va para Inicio e escaneie uma pasta.</p>}
    </div>
  );
}

function StatCard({ icon, label, value, color }: { icon: React.ReactNode; label: string; value: number; color: string }) {
  return <div className={"stat-card stat-" + color}><div className="stat-icon">{icon}</div><div className="stat-value">{value}</div><div className="stat-label">{label}</div></div>;
}

// Shared formatting helpers
const fmtClock = (ms: number) => { const total = Math.floor(ms / 1000); const h = Math.floor(total / 3600); const m = Math.floor((total % 3600) / 60); const s = total % 60; const mm = String(m).padStart(2, "0"); const ss = String(s).padStart(2, "0"); return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`; };
const fmtDate = (d: string | null | undefined) => { if (!d) return ""; try { return new Date(d).toLocaleDateString("pt-BR") + " " + new Date(d).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" }); } catch { return d; } };
const fmtSize = (b: number) => { if (b < 1024) return b + " B"; if (b < 1048576) return (b / 1024).toFixed(1) + " KB"; return (b / 1048576).toFixed(1) + " MB"; };
const durationOf = (t: TranscriptionView) => t.durationMs ?? (t.segments.length ? t.segments[t.segments.length - 1].endMs : 0);
const fileDateOf = (t: TranscriptionView) => t.createdAt || t.modifiedAt;
const textOf = (t: TranscriptionView) => (t.editedText?.trim() || t.rawText);

// Portuguese stopwords (also filters words with <= 4 letters at call site)
const STOPWORDS = new Set([
  "que", "para", "como", "mais", "mas", "foi", "ele", "ela", "isso", "esse", "essa",
  "este", "esta", "uma", "umas", "uns", "com", "sem", "por", "dos", "das", "nos", "nas",
  "aos", "pelo", "pela", "seu", "sua", "meu", "minha", "voce", "vocs", "voces", "entao",
  "tambem", "ainda", "quando", "porque", "depois", "sobre", "muito", "muita", "todos",
  "todas", "tudo", "nao", "sim", "aqui", "agora", "tem", "ter", "ser", "estar", "deste",
  "desta", "nesse", "nessa", "aquele", "aquela", "qual", "quem", "onde", "assim", "cada",
]);

function ReviewView({ jobs, selectedJob, transcription, onViewJob, onBack }: any) {
  const [items, setItems] = useState<TranscriptionView[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [ext, setExt] = useState("");
  const [sort, setSort] = useState("recent");
  const [minSize, setMinSize] = useState(0);

  const load = useCallback(async () => {
    setLoading(true);
    try { setItems(await invoke<TranscriptionView[]>("list_transcriptions")); }
    catch { setItems([]); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { load(); }, [load, jobs.length]);

  const extensions = useMemo(() => Array.from(new Set(items.map(t => t.extension.toLowerCase()))).sort(), [items]);

  const topWords = useMemo(() => {
    const freq: Record<string, number> = {};
    for (const t of items) {
      for (const w of textOf(t).toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").split(/[^a-z0-9]+/)) {
        if (w.length > 4 && !STOPWORDS.has(w)) freq[w] = (freq[w] || 0) + 1;
      }
    }
    return Object.entries(freq).sort((a, b) => b[1] - a[1]).slice(0, 10);
  }, [items]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    let list = items.filter(t => {
      if (ext && t.extension.toLowerCase() !== ext) return false;
      if (minSize && t.sizeBytes < minSize) return false;
      if (q && !textOf(t).toLowerCase().includes(q) && !t.fileName.toLowerCase().includes(q)) return false;
      return true;
    });
    const dateVal = (t: TranscriptionView) => { const d = fileDateOf(t); return d ? new Date(d).getTime() : 0; };
    list = [...list].sort((a, b) => {
      switch (sort) {
        case "oldest": return dateVal(a) - dateVal(b);
        case "name": return a.fileName.localeCompare(b.fileName);
        case "size": return b.sizeBytes - a.sizeBytes;
        case "duration": return durationOf(b) - durationOf(a);
        default: return dateVal(b) - dateVal(a);
      }
    });
    return list;
  }, [items, search, ext, minSize, sort]);

  const summary = useMemo(() => ({
    count: items.length,
    duration: items.reduce((acc, t) => acc + durationOf(t), 0),
    size: items.reduce((acc, t) => acc + t.sizeBytes, 0),
  }), [items]);

  if (selectedJob && transcription) return <TranscriptionDetail transcription={transcription} onBack={onBack} />;

  const hasFilters = !!(search || ext || minSize);

  return (
    <div className="view review-view">
      <div className="review-head">
        <h2>Revisao</h2>
        <button type="button" className="btn-ghost" onClick={load}><RefreshCw size={14} /> Atualizar</button>
      </div>

      {!loading && items.length > 0 && (
        <div className="review-summary">
          <div className="rs-card"><FileAudio size={16} /><div><strong>{summary.count}</strong><span>Transcricoes</span></div></div>
          <div className="rs-card"><Clock size={16} /><div><strong>{fmtClock(summary.duration)}</strong><span>Duracao total</span></div></div>
          <div className="rs-card"><HardDrive size={16} /><div><strong>{fmtSize(summary.size)}</strong><span>Tamanho total</span></div></div>
        </div>
      )}

      {topWords.length > 0 && (
        <div className="card top-words-card">
          <h3><BarChart3 size={14} /> Top 10 palavras mais identificadas</h3>
          <div className="top-words">
            {topWords.map(([word, count]) => (
              <button type="button" key={word} className="word-chip" onClick={() => setSearch(word)}>
                {word} <small>{count}</small>
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="review-toolbar">
        <div className="field-row tb-search">
          <Search size={15} className="tb-search-icon" />
          <input type="text" placeholder="Buscar texto em todas as transcricoes..."
            value={search} onChange={e => setSearch(e.target.value)} />
          {search && <button type="button" className="tb-clear" onClick={() => setSearch("")}><X size={14} /></button>}
        </div>
        <select value={ext} onChange={e => setExt(e.target.value)} title="Formato">
          <option value="">Todos formatos</option>
          {extensions.map(x => <option key={x} value={x}>{x.toUpperCase()}</option>)}
        </select>
        <select value={String(minSize)} onChange={e => setMinSize(Number(e.target.value))} title="Tamanho minimo">
          <option value="0">Qualquer tamanho</option>
          <option value={String(1048576)}>&gt; 1 MB</option>
          <option value={String(5242880)}>&gt; 5 MB</option>
          <option value={String(10485760)}>&gt; 10 MB</option>
        </select>
        <select value={sort} onChange={e => setSort(e.target.value)} title="Ordenar">
          <option value="recent">Mais recentes</option>
          <option value="oldest">Mais antigos</option>
          <option value="name">Nome (A-Z)</option>
          <option value="size">Maior tamanho</option>
          <option value="duration">Maior duracao</option>
        </select>
      </div>

      {hasFilters && (
        <div className="review-count">{filtered.length} de {items.length} resultado(s)
          <button type="button" className="btn-link" onClick={() => { setSearch(""); setExt(""); setMinSize(0); }}>limpar filtros</button>
        </div>
      )}

      {loading ? <p className="empty-hint">Carregando...</p> :
        items.length === 0 ? <p className="empty-hint">Nenhuma transcricao concluida.</p> :
        filtered.length === 0 ? <p className="empty-hint">Nenhum resultado para os filtros aplicados.</p> :
        <div className="review-cards">
          {filtered.map(t => (
            <FileCard key={t.transcriptionId} transcription={t} onViewJob={onViewJob} jobs={jobs} query={search.trim()} />
          ))}
        </div>}
    </div>
  );
}

function FileCard({ transcription, onViewJob, jobs, query }: { transcription: TranscriptionView; onViewJob: (j: JobRow) => void; jobs: JobRow[]; query?: string }) {
  const [showSegments, setShowSegments] = useState(false);
  const job = jobs.find(j => j.jobId === transcription.jobId);
  const dur = durationOf(transcription);
  const text = textOf(transcription);
  const fileDate = fileDateOf(transcription);
  const highlight = (t: string, q?: string) => {
    if (!q) return t;
    const re = new RegExp(`(${q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi");
    return t.split(re).map((p, i) => (i % 2 === 1 ? <mark key={i}>{p}</mark> : p));
  };
  const preview = text.length > 400 && !showSegments ? text.slice(0, 400) + "..." : text;

  return (
    <div className="file-card">
      <div className="fc-header" onClick={() => { if (job) onViewJob(job); }} role="button" tabIndex={0}
        onKeyDown={e => { if (e.key === "Enter" && job) onViewJob(job); }}>
        <div className="fc-meta">
          <span className="fc-name" title={transcription.relativePath}>{transcription.fileName}</span>
          <div className="fc-tags">
            <span className="fc-tag">{transcription.extension.toUpperCase()}</span>
            <span className="fc-tag"><HardDrive size={11} /> {fmtSize(transcription.sizeBytes)}</span>
            {dur > 0 && <span className="fc-tag"><Clock size={11} /> {fmtClock(dur)}</span>}
            {fileDate && <span className="fc-tag"><Calendar size={11} /> {fmtDate(fileDate)}</span>}
          </div>
        </div>
        <JobStatusBadge status="completed" />
      </div>

      <div className="fc-text" onClick={() => setShowSegments(!showSegments)}>
        {highlight(preview, query)}
      </div>

      {showSegments && transcription.segments.length > 0 && (
        <div className="fc-segments">
          {transcription.segments.map(seg => (
            <div key={seg.id} className="segment-row">
              <span className="seg-time">{fmtClock(seg.startMs)} - {fmtClock(seg.endMs)}</span>
              <strong>{highlight(seg.editedText?.trim() || seg.rawText, query)}</strong>
            </div>
          ))}
        </div>
      )}

      <button type="button" className="btn-expand" onClick={() => setShowSegments(!showSegments)}>
        {showSegments ? "Recolher segmentos" : `Ver ${transcription.segments.length} segmentos`}
      </button>
    </div>
  );
}

function TranscriptionDetail({ transcription, onBack }: { transcription: TranscriptionView; onBack: () => void }) {
  const [search, setSearch] = useState(""); const [audioUrl, setAudioUrl] = useState("");
  const [showSegments, setShowSegments] = useState(false);
  const text = textOf(transcription);
  const dur = durationOf(transcription);
  const fileDate = fileDateOf(transcription);

  useEffect(() => {
    invoke<string>("read_audio", { path: transcription.absolutePath }).then(setAudioUrl).catch(() => {});
  }, [transcription.absolutePath]);

  return (
    <div className="view transcription-detail">
      <button type="button" className="btn-back" onClick={onBack}><ArrowLeft size={16} /> Voltar</button>
      <h2>{transcription.fileName}</h2>

      <div className="fc-meta-bar">
        <span className="fc-tag">{transcription.extension.toUpperCase()}</span>
        <span className="fc-tag"><HardDrive size={11} /> {fmtSize(transcription.sizeBytes)}</span>
        {dur > 0 && <span className="fc-tag"><Clock size={11} /> {fmtClock(dur)}</span>}
        {fileDate && <span className="fc-tag"><Calendar size={11} /> {fmtDate(fileDate)}</span>}
      </div>

      <div className="field" style={{ margin: "10px 0" }}>
        <div className="field-row">
          <input type="text" placeholder="Buscar nos segmentos..." value={search} onChange={e => setSearch(e.target.value)} />
          <button type="button" disabled><Search size={14} /></button>
        </div>
      </div>

      {audioUrl && <audio controls src={audioUrl} style={{ width: "100%", marginBottom: 10 }} />}

      <div className="fc-text-full">{text}</div>

      <button type="button" className="btn-expand" onClick={() => setShowSegments(!showSegments)} style={{ marginTop: 8 }}>
        {showSegments ? "Ocultar segmentos" : "Ver segmentos"}
      </button>

      {showSegments && transcription.segments.length > 0 && (
        <div className="fc-segments" style={{ marginTop: 8 }}>
          {(search.trim() ? transcription.segments.filter(s => s.rawText.toLowerCase().includes(search.toLowerCase())) : transcription.segments)
            .map(seg => (
              <div key={seg.id} className="segment-row">
                <span className="seg-time">{fmtClock(seg.startMs)} - {fmtClock(seg.endMs)}</span>
                <strong>{seg.rawText}</strong>
              </div>
            ))}
        </div>
      )}
    </div>
  );
}

function SettingsView({ profiles, activeProfile, onProfilesChanged }: { profiles: ProfileRow[]; activeProfile: ProfileRow | null; onProfilesChanged: () => void }) {
  const [form, setForm] = useState<ProfileRow>({ ...EMPTY_PROFILE });
  async function chooseModel() { try { if (form.backend === "faster_whisper") { const s = await open({ directory: true, multiple: false }); if (typeof s === "string") setForm(f => ({ ...f, modelPath: s })); } else { const s = await open({ multiple: false, filters: [{ name: "Modelo", extensions: ["bin", "ggml"] }] }); if (typeof s === "string") setForm(f => ({ ...f, modelPath: s })); } } catch { } }
  async function save() { if (!form.name.trim() || !form.modelPath.trim()) return; try { await invoke("save_profile", { profile: form }); onProfilesChanged(); setForm({ ...EMPTY_PROFILE }); } catch { } }
  async function select(p: ProfileRow) { try { await invoke("set_active_profile", { profile: p }); onProfilesChanged(); } catch { } }
  async function del(id: number) { try { await invoke("delete_profile", { id }); onProfilesChanged(); } catch { } }
  return (
    <div className="view settings-view"><h2>Configuracoes</h2>
      {profiles.length > 0 && <section className="card"><h3>Perfis salvos</h3><div className="profile-list">{profiles.map((p: ProfileRow) => (<div key={p.id} className={"profile-card" + (activeProfile?.id === p.id ? " profile-active" : "")}><div className="profile-card-info"><strong>{p.name} {activeProfile?.id === p.id && <Check size={12} />}</strong><span>{p.backend} · {p.device} · {p.threads} threads · {p.precision}</span><span className="profile-model-path">{p.modelPath}</span></div><div className="profile-card-actions">{activeProfile?.id !== p.id && <button type="button" onClick={() => select(p)}>Usar</button>}<button type="button" className="btn-danger" onClick={() => del(p.id)}><Trash2 size={14} /></button></div></div>))}</div></section>}
      <section className="card"><h3>Novo perfil</h3>
        <label className="field">Nome<input type="text" value={form.name} onChange={e => setForm({ ...form, name: e.target.value })} placeholder="Meu modelo" /></label>
        <label className="field">Backend<div className="pill-group"><button type="button" className={"pill" + (form.backend === "faster_whisper" ? " pill-active" : "")} onClick={() => setForm({ ...form, backend: "faster_whisper" })}>faster-whisper</button><button type="button" className={"pill" + (form.backend === "whisper_cpp" ? " pill-active" : "")} onClick={() => setForm({ ...form, backend: "whisper_cpp" })}>whisper.cpp</button></div></label>
        <label className="field">Modelo<div className="field-row"><input type="text" value={form.modelPath} onChange={e => setForm({ ...form, modelPath: e.target.value })} placeholder={form.backend === "faster_whisper" ? "Pasta do modelo CTranslate2" : "Arquivo .bin"} /><button type="button" onClick={chooseModel}>...</button></div></label>
        <label className="field">Dispositivo<div className="pill-group"><button type="button" className={"pill" + (form.device === "cpu" ? " pill-active" : "")} onClick={() => setForm({ ...form, device: "cpu" })}>CPU</button><button type="button" className={"pill" + (form.device === "cuda" ? " pill-active" : "")} onClick={() => setForm({ ...form, device: "cuda" })}>CUDA</button><button type="button" className={"pill" + (form.device === "auto" ? " pill-active" : "")} onClick={() => setForm({ ...form, device: "auto" })}>Auto</button></div></label>
        <div className="field-grid">
          <label className="field">Precisao<select value={form.precision} onChange={e => setForm({ ...form, precision: e.target.value })}><option value="auto">Auto - o modelo decide</option><option value="float16">FP16 - melhor GPU</option><option value="int8_float16">INT8 FP16 - equilibrado</option><option value="int8">INT8 - pouca memoria</option></select></label>
          <label className="field">Threads<select value={form.threads} onChange={e => setForm({ ...form, threads: Number(e.target.value) })}>{[1, 2, 4, 6, 8, 12, 16, 24, 32].map(n => <option key={n} value={n}>{n} threads</option>)}</select></label>
          <label className="field">Idioma<input type="text" value={form.language ?? ""} onChange={e => setForm({ ...form, language: e.target.value || null })} placeholder="pt / en / auto" /></label>
          <label className="field">Tarefa<select value={form.task} onChange={e => setForm({ ...form, task: e.target.value })}><option value="transcribe">Transcrever</option><option value="translate">Traduzir</option></select></label>
        </div>
        <button type="button" className="btn-save" onClick={save}><Plus size={14} /> Salvar perfil</button>
      </section>
    </div>
  );
}

function JobStatusBadge({ status }: { status: string }) {
  const map: Record<string, { label: string; cls: string }> = { pending: { label: "Pendente", cls: "status-pending" }, processing: { label: "Processando", cls: "status-processing" }, completed: { label: "Concluido", cls: "status-completed" }, error: { label: "Erro", cls: "status-error" }, reviewed: { label: "Revisado", cls: "status-reviewed" }, exported: { label: "Exportado", cls: "status-exported" } };
  const info = map[status] ?? { label: status, cls: "" }; return <span className={"status-badge " + info.cls}>{info.label}</span>;
}

function formatError(error: unknown) { if (error instanceof Error) return error.message; if (typeof error === "string") return error; return "erro desconhecido"; }
