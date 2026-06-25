"""Reparo do banco do Transcrever.

Remove jobs duplicados deixados por versoes antigas, mantendo, por arquivo,
apenas UM job na seguinte ordem de prioridade:

    concluido (completed) > erro (error) > processando (processing) > pendente (pending)

Transcricoes e segmentos dos jobs descartados tambem sao removidos. Os arquivos
de audio no disco NUNCA sao tocados.

Uso:
    python scripts/repair_db.py                # tenta o caminho padrao (Windows)
    python scripts/repair_db.py "C:\\caminho\\transcrever.sqlite"
"""

import os
import sqlite3
import sys


def default_db_path() -> str:
    appdata = os.environ.get("APPDATA")
    if appdata:
        return os.path.join(appdata, "br.local.transcrever", "transcrever.sqlite")
    return ""


def main() -> int:
    db_path = sys.argv[1] if len(sys.argv) > 1 else default_db_path()
    if not db_path or not os.path.isfile(db_path):
        print(f"Banco nao encontrado: {db_path!r}")
        print("Passe o caminho do arquivo .sqlite como argumento.")
        return 1

    print(f"Abrindo: {db_path}")
    # Backup de seguranca antes de mexer.
    backup = db_path + ".bak"
    try:
        import shutil
        shutil.copyfile(db_path, backup)
        print(f"Backup criado: {backup}")
    except Exception as exc:  # noqa: BLE001
        print(f"Aviso: nao foi possivel criar backup ({exc}). Continuando.")

    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA foreign_keys = OFF")
    cur = conn.cursor()

    total_jobs = cur.execute("SELECT COUNT(*) FROM transcription_jobs").fetchone()[0]

    # Ids dos jobs duplicados (todos menos o "melhor" por media_file_id).
    dup_ids = [row[0] for row in cur.execute(
        """
        SELECT id FROM (
            SELECT id, ROW_NUMBER() OVER (
                PARTITION BY media_file_id
                ORDER BY CASE status
                    WHEN 'completed' THEN 0 WHEN 'error' THEN 1
                    WHEN 'processing' THEN 2 WHEN 'pending' THEN 3 ELSE 4 END,
                    id DESC
            ) AS rn
            FROM transcription_jobs
        ) WHERE rn > 1
        """
    ).fetchall()]

    print(f"Jobs no total: {total_jobs}")
    print(f"Jobs duplicados a remover: {len(dup_ids)}")

    for jid in dup_ids:
        cur.execute(
            "DELETE FROM transcription_segments WHERE transcription_id IN "
            "(SELECT id FROM transcriptions WHERE job_id = ?)",
            (jid,),
        )
        cur.execute("DELETE FROM transcriptions WHERE job_id = ?", (jid,))
        cur.execute("DELETE FROM transcription_jobs WHERE id = ?", (jid,))

    # Qualquer job preso em 'processing' (de um fechamento anterior) volta a pendente.
    reset = cur.execute(
        "UPDATE transcription_jobs SET status='pending', started_at=NULL WHERE status='processing'"
    ).rowcount

    conn.commit()
    cur.execute("VACUUM")
    conn.commit()

    remaining = cur.execute("SELECT COUNT(*) FROM transcription_jobs").fetchone()[0]
    by_status = dict(cur.execute(
        "SELECT status, COUNT(*) FROM transcription_jobs GROUP BY status"
    ).fetchall())
    conn.close()

    print(f"Jobs 'processing' reenfileirados: {reset}")
    print(f"Jobs restantes: {remaining}")
    print(f"Por status: {by_status}")
    print("Concluido.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
