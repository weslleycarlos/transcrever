import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Download, FolderOpen, Play, Settings, Plus, Trash2, Check, AlertCircle, Loader, Clock, ArrowLeft } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { JobRow, ProfileRow, ScanResponse, TranscriptionView } from "./types";

const EMPTY_PROFILE: ProfileRow = {
  id: 0,
  name: "",
  backend: "whisper_cpp",
  modelPath: "",
  device: "cpu",
  precision: "fp16",
  threads: 4,
  language: "pt",
  task: "transcribe",
  advancedJson: "{}",
};

export default function App() {
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
  const [message, setMessage] = useState("");
  const [queuedCount, setQueuedCount] = useState(0);
  const [showProfile, setShowProfile] = useState(false);
  const [profile, setProfile] = useState<ProfileRow>({ ...EMPTY_PROFILE });
  const [profiles, setProfiles] = useState<ProfileRow[]>([]);
  const [activeProfile, setActiveProfile] = useState<ProfileRow | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [jobs, setJobs] = useState<JobRow[]>([]);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const [selectedJob, setSelectedJob] = useState<JobRow | null>(null);
  const [transcription, setTranscription] = useState<TranscriptionView | null>(null);

  const canStart = queuedCount > 0 && activeProfile !== null && !isRunning;

  useEffect(() => {
    loadProfiles();
    loadActiveProfile();
    return () => stopPolling();
  }, []);

  const startPolling = useCallback(() => {
    if (pollingRef.current) return;
    pollingRef.current = setInterval(async () => {
      try {
        const list = await invoke<JobRow[]>("list_jobs");
        setJobs(list);
        const pending = list.filter((j) => j.status === "pending" || j.status === "processing");
        if (pending.length === 0 && list.length > 0) {
          setIsRunning(false);
          stopPolling();
          setMessage("Transcricao concluida.");
        }
      } catch {
        // ignore polling errors
      }
    }, 1500);
  }, []);

  const stopPolling = () => {
    if (pollingRef.current) {
      clearInterval(pollingRef.current);
      pollingRef.current = null;
    }
  };

  async function loadProfiles() {
    try {
      const list = await invoke<ProfileRow[]>("list_profiles");
      setProfiles(list);
    } catch {
      // profiles not available yet
    }
  }

  async function loadActiveProfile() {
    try {
      const active = await invoke<ProfileRow | null>("get_active_profile");
      setActiveProfile(active);
    } catch {
      // not available yet
    }
  }

  async function chooseSource() {
    try {
      const selected = await open({ directory: true, multiple: false });

      if (typeof selected !== "string") {
        return;
      }

      const response = await invoke<ScanResponse>("scan_source_folder", { path: selected });
      setSource(selected);
      setQueuedCount(response.queuedCount);
      setMessage(`${response.discoveredCount} arquivos encontrados, ${response.queuedCount} jobs criados.`);
    } catch (error) {
      setMessage(`Nao foi possivel carregar a pasta de origem: ${formatError(error)}`);
    }
  }

  async function chooseDestination() {
    try {
      const selected = await open({ directory: true, multiple: false });

      if (typeof selected !== "string") {
        return;
      }

      await invoke<void>("set_export_folder", { path: selected });
      setDestination(selected);
      setMessage("Pasta de destino configurada.");
    } catch (error) {
      setMessage(`Nao foi possivel configurar a pasta de destino: ${formatError(error)}`);
    }
  }

  async function chooseModelPath() {
    try {
      if (profile.backend === "faster_whisper") {
        const selected = await open({ directory: true, multiple: false });
        if (typeof selected === "string") {
          setProfile((p) => ({ ...p, modelPath: selected }));
        }
      } else {
        const selected = await open({
          multiple: false,
          filters: [{ name: "Modelo", extensions: ["bin", "ggml"] }],
        });
        if (typeof selected === "string") {
          setProfile((p) => ({ ...p, modelPath: selected }));
        }
      }
    } catch {
      // user cancelled
    }
  }

  async function handleSaveProfile() {
    try {
      if (!profile.name.trim() || !profile.modelPath.trim()) {
        setMessage("Preencha o nome e o caminho do modelo.");
        return;
      }
      const saved = await invoke<ProfileRow>("save_profile", { profile });
      setActiveProfile(saved);
      setMessage(`Perfil "${saved.name}" salvo e ativado.`);
      setShowProfile(false);
      await loadProfiles();
    } catch (error) {
      setMessage(`Erro ao salvar perfil: ${formatError(error)}`);
    }
  }

  async function handleSelectProfile(p: ProfileRow) {
    try {
      await invoke("set_active_profile", { profile: p });
      setActiveProfile(p);
      setMessage(`Perfil "${p.name}" ativado.`);
    } catch (error) {
      setMessage(`Erro ao ativar perfil: ${formatError(error)}`);
    }
  }

  async function handleDeleteProfile(id: number) {
    try {
      await invoke("delete_profile", { id });
      if (activeProfile?.id === id) setActiveProfile(null);
      await loadProfiles();
    } catch (error) {
      setMessage(`Erro ao remover perfil: ${formatError(error)}`);
    }
  }

  async function handleStart() {
    if (!canStart) return;
    try {
      await invoke("start_transcription");
      setIsRunning(true);
      setMessage("Transcricao em andamento...");
      startPolling();
    } catch (error) {
      setMessage(`Erro ao iniciar: ${formatError(error)}`);
    }
  }

  async function handleViewJob(job: JobRow) {
    if (job.status !== "completed") return;
    setSelectedJob(job);
    try {
      const result = await invoke<TranscriptionView | null>("get_transcription", { jobId: job.jobId });
      setTranscription(result);
    } catch (error) {
      setMessage(`Erro ao carregar transcricao: ${formatError(error)}`);
    }
  }

  function handleBackToQueue() {
    setSelectedJob(null);
    setTranscription(null);
  }

  const pendingCount = jobs.filter((j) => j.status === "pending").length;
  const processingCount = jobs.filter((j) => j.status === "processing").length;
  const errorCount = jobs.filter((j) => j.status === "error").length;
  const doneCount = jobs.filter((j) => j.status === "completed").length;

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <h1>Transcrever</h1>
        <button type="button" onClick={chooseSource}>
          <FolderOpen size={16} aria-hidden="true" />
          Origem
        </button>
        <div className={source ? "path-box" : "path-box path-box-empty"}>{source || "Origem nao selecionada"}</div>
        <button type="button" onClick={chooseDestination}>
          <Download size={16} aria-hidden="true" />
          Destino
        </button>
        <div className={destination ? "path-box" : "path-box path-box-empty"}>
          {destination || "Destino nao selecionado"}
        </div>
        <button type="button" onClick={() => setShowProfile(true)}>
          <Settings size={16} aria-hidden="true" />
          Perfil
        </button>
        {activeProfile && (
          <div className="path-box" style={{ borderColor: "#22c55e" }}>
            <Check size={12} /> {activeProfile.name}
          </div>
        )}
      </aside>
      <section className="workspace">
        {showProfile ? (
          <ProfilePanel
            profile={profile}
            profiles={profiles}
            onChange={setProfile}
            onSave={handleSaveProfile}
            onChooseModel={chooseModelPath}
            onSelectProfile={handleSelectProfile}
            onDeleteProfile={handleDeleteProfile}
            onClose={() => setShowProfile(false)}
          />
        ) : selectedJob ? (
          <ReviewPanel
            job={selectedJob}
            transcription={transcription}
            onBack={handleBackToQueue}
          />
        ) : (
          <>
            <header className="toolbar">
              <h2>Fila {jobs.length > 0 && <span className="badge">{jobs.length}</span>}</h2>
              <button type="button" disabled={!canStart} onClick={handleStart}>
                {isRunning ? <Loader size={16} className="spin" /> : <Play size={16} />}
                {isRunning ? "Processando..." : "Iniciar"}
              </button>
            </header>

            <div className="queue-summary">{message || "A fila ainda esta vazia."}</div>

            {isRunning && (
              <div className="progress-bar-container">
                <div className="progress-bar" style={{ width: `${jobs.length > 0 ? ((doneCount + errorCount) / jobs.length) * 100 : 0}%` }} />
              </div>
            )}

            <div className="stats-row">
              {pendingCount > 0 && <span className="stat stat-pending"><Clock size={12} /> {pendingCount} pendentes</span>}
              {processingCount > 0 && <span className="stat stat-processing"><Loader size={12} className="spin" /> {processingCount} processando</span>}
              {doneCount > 0 && <span className="stat stat-done"><Check size={12} /> {doneCount} concluidos</span>}
              {errorCount > 0 && <span className="stat stat-error"><AlertCircle size={12} /> {errorCount} erros</span>}
            </div>

            {jobs.length > 0 && (
              <div className="job-list">
                {jobs.map((job) => (
                  <div
                    key={job.jobId}
                    className={`job-row job-${job.status}${job.status === "completed" ? " job-clickable" : ""}`}
                    onClick={() => handleViewJob(job)}
                    role={job.status === "completed" ? "button" : undefined}
                    tabIndex={job.status === "completed" ? 0 : undefined}
                    onKeyDown={(e) => { if (e.key === "Enter" && job.status === "completed") handleViewJob(job); }}
                  >
                    <div className="job-info">
                      <span className="job-name">{job.fileName}</span>
                      <span className="job-path">{job.relativePath}</span>
                    </div>
                    <div className="job-status-area">
                      <JobStatusBadge status={job.status} />
                      {job.errorMessage && (
                        <span className="job-error" title={job.errorMessage}>{job.errorMessage}</span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {!isRunning && jobs.length === 0 && (
              <section className="review-layout">
                <div className="segments-panel">
                  <h3>Segmentos</h3>
                  <p className="empty-hint">Selecione uma origem e inicie a transcricao.</p>
                </div>
                <div className="text-panel">
                  <h3>Texto continuo</h3>
                  <p className="empty-hint">O texto aparecera aqui apos a transcricao.</p>
                </div>
              </section>
            )}
          </>
        )}
      </section>
    </main>
  );
}

function JobStatusBadge({ status }: { status: string }) {
  const map: Record<string, { label: string; className: string }> = {
    pending: { label: "Pendente", className: "status-pending" },
    processing: { label: "Processando", className: "status-processing" },
    completed: { label: "Concluido", className: "status-completed" },
    error: { label: "Erro", className: "status-error" },
    reviewed: { label: "Revisado", className: "status-reviewed" },
    exported: { label: "Exportado", className: "status-exported" },
  };
  const info = map[status] ?? { label: status, className: "" };
  return <span className={`status-badge ${info.className}`}>{info.label}</span>;
}

function ProfilePanel({
  profile,
  profiles,
  onChange,
  onSave,
  onChooseModel,
  onSelectProfile,
  onDeleteProfile,
  onClose,
}: {
  profile: ProfileRow;
  profiles: ProfileRow[];
  onChange: (p: ProfileRow) => void;
  onSave: () => void;
  onChooseModel: () => void;
  onSelectProfile: (p: ProfileRow) => void;
  onDeleteProfile: (id: number) => void;
  onClose: () => void;
}) {
  return (
    <div className="profile-panel">
      <button type="button" className="btn-back" onClick={onClose}>
        <ArrowLeft size={16} /> Voltar para a fila
      </button>
      <h2>Configuracao do Perfil de Transcricao</h2>

      {profiles.length > 0 && (
        <section className="profile-section">
          <h3>Perfis salvos</h3>
          <div className="profile-list">
            {profiles.map((p) => (
              <div key={p.id} className="profile-card">
                <div className="profile-card-info">
                  <strong>{p.name}</strong>
                  <span>
                    {p.backend} &middot; {p.device} &middot; {p.threads} threads
                  </span>
                  <span className="profile-model-path">{p.modelPath}</span>
                </div>
                <div className="profile-card-actions">
                  <button type="button" onClick={() => onSelectProfile(p)}>
                    <Check size={14} /> Usar
                  </button>
                  <button type="button" className="btn-danger" onClick={() => onDeleteProfile(p.id)}>
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      <section className="profile-section">
        <h3>{profile.id ? `Editar: ${profile.name}` : "Novo perfil"}</h3>

        <label className="field">
          Nome do perfil
          <input
            type="text"
            value={profile.name}
            onChange={(e) => onChange({ ...profile, name: e.target.value })}
            placeholder="Ex: Meu Modelo Local"
          />
        </label>

        <label className="field">
          Caminho do modelo
          <div className="field-row">
            <input
              type="text"
              value={profile.modelPath}
              onChange={(e) => onChange({ ...profile, modelPath: e.target.value })}
              placeholder="C:\\modelos\\ggml-model.bin"
            />
            <button type="button" onClick={onChooseModel}>
              ...
            </button>
          </div>
        </label>

        <div className="field-grid">
          <label className="field">
            Backend
            <select value={profile.backend} onChange={(e) => onChange({ ...profile, backend: e.target.value })}>
              <option value="whisper_cpp">whisper.cpp</option>
              <option value="faster_whisper">faster-whisper</option>
            </select>
          </label>

          <label className="field">
            Dispositivo
            <select value={profile.device} onChange={(e) => onChange({ ...profile, device: e.target.value })}>
              <option value="cpu">CPU</option>
              <option value="cuda">CUDA</option>
            </select>
          </label>

          <label className="field">
            Precisao
            <select value={profile.precision} onChange={(e) => onChange({ ...profile, precision: e.target.value })}>
              <option value="fp16">FP16</option>
              <option value="fp32">FP32</option>
              <option value="int8">INT8</option>
            </select>
          </label>

          <label className="field">
            Threads
            <input
              type="number"
              min={1}
              max={32}
              value={profile.threads}
              onChange={(e) => onChange({ ...profile, threads: Number(e.target.value) || 4 })}
            />
          </label>

          <label className="field">
            Idioma (opcional)
            <input
              type="text"
              value={profile.language ?? ""}
              onChange={(e) => onChange({ ...profile, language: e.target.value || null })}
              placeholder="pt / en / auto"
            />
          </label>

          <label className="field">
            Tarefa
            <select value={profile.task} onChange={(e) => onChange({ ...profile, task: e.target.value })}>
              <option value="transcribe">Transcrever</option>
              <option value="translate">Traduzir</option>
            </select>
          </label>
        </div>

        <button type="button" className="btn-primary" onClick={onSave}>
          <Plus size={16} /> Salvar perfil
        </button>
      </section>
    </div>
  );
}

function formatError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "erro desconhecido";
}

function ReviewPanel({
  job,
  transcription,
  onBack,
}: {
  job: JobRow;
  transcription: TranscriptionView | null;
  onBack: () => void;
}) {
  function formatMs(ms: number) {
    const totalSec = Math.floor(ms / 1000);
    const min = Math.floor(totalSec / 60);
    const sec = totalSec % 60;
    return `${String(min).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
  }

  if (!transcription) {
    return (
      <div className="review-panel">
        <button type="button" className="btn-back" onClick={onBack}>
          <ArrowLeft size={16} /> Voltar para a fila
        </button>
        <p>Carregando transcricao...</p>
      </div>
    );
  }

  const editedText = transcription.editedText || "";
  const displayText = editedText.trim() || transcription.rawText;

  return (
    <div className="review-panel">
      <button type="button" className="btn-back" onClick={onBack}>
        <ArrowLeft size={16} /> Voltar para a fila
      </button>
      <h2>{transcription.fileName}</h2>

      <div className="review-layout">
        <div className="segments-panel">
          <h3>Segmentos</h3>
          <div className="segments-list">
            {transcription.segments.map((seg) => (
              <button type="button" key={seg.id} className="segment-row">
                <span>{formatMs(seg.startMs)} - {formatMs(seg.endMs)}</span>
                <strong>{seg.rawText}</strong>
              </button>
            ))}
          </div>
        </div>
        <div className="text-panel">
          <h3>Texto continuo</h3>
          <textarea defaultValue={displayText} readOnly />
          <audio controls src={`https://asset.localhost/${encodeURIComponent(transcription.absolutePath)}`} />
        </div>
      </div>
    </div>
  );
}
