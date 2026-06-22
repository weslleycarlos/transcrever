export default function App() {
  return (
    <main className="app-shell">
      <aside className="sidebar">
        <h1>Transcrever</h1>
        <button type="button">Origem</button>
        <button type="button">Destino</button>
        <button type="button">Perfil</button>
      </aside>
      <section className="workspace">
        <header className="toolbar">
          <h2>Fila</h2>
          <button type="button">Iniciar</button>
        </header>
        <div className="empty-state">Selecione uma pasta de origem para montar a fila.</div>
      </section>
    </main>
  );
}
