# Transcricao Desktop Rust Design

## Objetivo

Criar um aplicativo desktop em Rust para transcrever lotes de audio e video a partir de uma pasta de origem, varrendo subpastas, salvando os resultados em SQLite e permitindo revisar a transcricao com player de audio antes de exportar arquivos `.txt`.

## Escopo Da Primeira Versao

A primeira versao sera um aplicativo desktop local para Windows, construido com Tauri e Rust. Ela deve permitir:

- Selecionar uma pasta de origem e varrer subpastas.
- Selecionar uma pasta de destino para exportacoes `.txt`.
- Detectar arquivos de audio e video suportados.
- Configurar parametros de desempenho e qualidade da transcricao.
- Processar uma fila local de arquivos.
- Salvar resultados em SQLite.
- Revisar transcricoes por segmentos com timestamps.
- Ver e editar tambem uma visao de texto continuo.
- Ouvir o arquivo original enquanto revisa a transcricao.
- Exportar transcricoes revisadas ou brutas para `.txt`, individualmente ou em lote.
- Retomar trabalhos interrompidos sem perder estado.

Ficam fora da primeira versao: colaboracao multiusuario, sincronizacao em nuvem, diarizacao de falantes, editor de legendas `.srt`, treinamento ou ajuste fino de modelos e processamento distribuido em varias maquinas.

## Arquitetura

O aplicativo sera dividido em quatro camadas principais:

1. Interface desktop Tauri
   - Seleciona pastas.
   - Mostra fila, progresso, filtros e erros.
   - Permite configurar perfis de transcricao.
   - Exibe player de audio.
   - Exibe segmentos editaveis e texto continuo.
   - Aciona exportacoes.

2. Core Rust
   - Orquestra varredura de arquivos.
   - Gerencia fila e estados.
   - Aplica configuracoes do usuario.
   - Chama o backend de transcricao.
   - Normaliza segmentos e grava no SQLite.

3. Backend de transcricao
   - A primeira implementacao usara `whisper.cpp`.
   - O backend sera acessado por uma interface interna `TranscriptionBackend`.
   - A interface deixara espaco para backends futuros, como ONNX, Candle ou outro carregador de modelos Hugging Face.

4. Persistencia SQLite
   - Guarda arquivos descobertos.
   - Guarda jobs da fila.
   - Guarda parametros usados.
   - Guarda segmentos originais e editados.
   - Guarda status de revisao e exportacao.

## Backend Whisper

O backend inicial sera baseado em `whisper.cpp`, pois oferece uma rota pratica para desempenho local em CPU e GPU, com suporte a modelos quantizados e boa compatibilidade no Windows.

O aplicativo deve aceitar modelos compatibilizados com esse backend, incluindo variantes do Whisper como `large-v3-turbo` quando disponiveis em formato aceito pelo `whisper.cpp`.

Parametros expostos na primeira versao:

- Modelo selecionado.
- Dispositivo preferido: CPU ou GPU quando disponivel.
- Tipo/quantizacao do modelo: por exemplo int8, quantizado do `whisper.cpp`, float16 ou float32 conforme suporte real do backend.
- Numero de threads.
- Idioma fixo ou autodeteccao.
- Modo transcrever ou traduzir.
- Tamanho de contexto, quando suportado.
- Beam size ou configuracao equivalente, quando suportado.
- Temperatura ou estrategia equivalente, quando suportado.

Quando um parametro nao for suportado pelo backend/modelo selecionado, a interface deve mostrar isso claramente e impedir uma configuracao invalida.

## Fluxo De Uso

1. O usuario abre o aplicativo.
2. Escolhe uma pasta de origem.
3. O aplicativo varre a pasta e subpastas em busca de midias suportadas.
4. O usuario escolhe ou confirma a pasta de exportacao.
5. O usuario escolhe um perfil de transcricao.
6. O aplicativo cria ou atualiza a fila no SQLite.
7. O usuario inicia o processamento.
8. Cada arquivo passa pelos estados: pendente, processando, concluido, erro, revisado e exportado.
9. Ao concluir uma transcricao, o aplicativo salva segmentos timestampados e texto agregado.
10. O usuario revisa ouvindo o audio e editando segmentos ou texto continuo.
11. O usuario exporta um ou varios arquivos `.txt`.

## Varredura De Arquivos

A varredura deve ser recursiva e preservar o caminho relativo de cada arquivo dentro da pasta de origem. Isso permite organizar exportacoes respeitando a estrutura original quando o usuario desejar.

Extensoes iniciais suportadas:

- Audio: `.mp3`, `.wav`, `.m4a`, `.flac`, `.ogg`, `.opus`, `.aac`, `.wma`.
- Video: `.mp4`, `.mkv`, `.mov`, `.avi`, `.webm`.

O aplicativo deve calcular uma assinatura para cada arquivo usando caminho absoluto, tamanho e data de modificacao. Hash completo do conteudo fica como melhoria futura, pois pode ser caro em lotes grandes.

## SQLite

O SQLite sera a fonte de verdade do aplicativo. Arquivos `.txt` serao exportacoes derivadas.

Tabelas principais:

- `media_files`
  - `id`
  - `source_root`
  - `absolute_path`
  - `relative_path`
  - `file_name`
  - `extension`
  - `size_bytes`
  - `modified_at`
  - `duration_ms`
  - `discovered_at`

- `transcription_jobs`
  - `id`
  - `media_file_id`
  - `status`
  - `profile_id`
  - `progress`
  - `error_message`
  - `created_at`
  - `started_at`
  - `finished_at`

- `transcription_profiles`
  - `id`
  - `name`
  - `backend`
  - `model_path`
  - `device`
  - `precision`
  - `threads`
  - `language`
  - `task`
  - `advanced_json`

- `transcriptions`
  - `id`
  - `media_file_id`
  - `job_id`
  - `raw_text`
  - `edited_text`
  - `is_reviewed`
  - `created_at`
  - `updated_at`

- `transcription_segments`
  - `id`
  - `transcription_id`
  - `segment_index`
  - `start_ms`
  - `end_ms`
  - `raw_text`
  - `edited_text`
  - `confidence`

- `exports`
  - `id`
  - `transcription_id`
  - `export_path`
  - `format`
  - `created_at`

## Revisao

A tela de revisao tera duas visoes:

1. Segmentos
   - Lista de trechos com timestamp inicial e final.
   - Texto editavel por segmento.
   - Clique em um segmento posiciona o player no inicio do trecho.
   - O trecho atual e destacado enquanto o audio toca.

2. Texto continuo
   - Texto editavel completo.
   - Pode ser regenerado a partir dos segmentos editados.
   - Pode ser usado para ajustes finais antes da exportacao.

O aplicativo deve preservar o texto bruto gerado pelo modelo e salvar as edicoes separadamente. Assim o usuario pode comparar ou restaurar quando necessario.

## Exportacao

A primeira versao exportara `.txt`.

Modos de exportacao:

- Arquivo individual.
- Todos os concluidos.
- Todos os revisados.
- Exportar preservando estrutura de subpastas.
- Exportar tudo em uma pasta unica, resolvendo nomes duplicados com sufixo numerico.

Conteudo exportado:

- Por padrao, usar `edited_text` quando existir.
- Caso contrario, gerar texto a partir dos segmentos editados.
- Caso nao existam edicoes, usar texto bruto.

## Erros E Recuperacao

O aplicativo deve registrar erro por arquivo sem interromper toda a fila. Exemplos:

- Arquivo inacessivel.
- Formato nao suportado.
- Modelo ausente.
- Backend sem suporte ao parametro escolhido.
- Falha de GPU.
- Falha no processo do `whisper.cpp`.

Quando houver falha de GPU, o aplicativo deve sugerir reprocessar em CPU ou com outro perfil. Jobs com erro podem ser reenfileirados.

## Desempenho

A primeira versao deve priorizar controle e previsibilidade:

- Processar um arquivo por vez por padrao.
- Permitir configurar paralelismo no futuro, mas nao ativar processamento paralelo inicialmente.
- Expor threads no perfil.
- Registrar tempo de processamento por arquivo.
- Mostrar velocidade relativa quando o backend fornecer dados suficientes.

## Testes

Testes unitarios:

- Varredura recursiva e filtros de extensao.
- Assinatura de arquivos.
- Criacao e atualizacao de jobs.
- Regras de escolha do texto exportado.
- Normalizacao de segmentos.

Testes de integracao:

- Criar banco SQLite temporario.
- Inserir arquivo, job, transcricao e segmentos.
- Exportar `.txt`.
- Simular job com erro e reenfileirar.

Teste manual obrigatorio:

- Rodar o app com uma pasta pequena de amostras.
- Transcrever pelo menos um audio curto.
- Revisar segmento.
- Editar texto continuo.
- Exportar `.txt`.
- Fechar e abrir novamente confirmando que o estado foi preservado.

## Criterios De Aceite

A primeira versao sera considerada pronta quando:

- O usuario conseguir selecionar origem e destino.
- O app encontrar midias em subpastas.
- O usuario conseguir configurar um perfil basico.
- O app conseguir transcrever pelo menos um arquivo usando `whisper.cpp`.
- O SQLite persistir fila, parametros, transcricao e segmentos.
- O usuario conseguir ouvir o audio e revisar a transcricao.
- O usuario conseguir exportar `.txt`.
- O app conseguir retomar estado apos reiniciar.
