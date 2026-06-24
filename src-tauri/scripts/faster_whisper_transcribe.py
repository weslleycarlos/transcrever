"""faster-whisper transcription adapter for Transcrever.

Usage:
    python faster_whisper_transcribe.py --model <dir> --audio <file> [options]

Outputs JSON with "raw_text" and "segments" to stdout.
"""

import argparse
import json
import sys
import time

# Force UTF-8 on Windows to avoid encoding issues with accented characters
if sys.platform == "win32":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")


def resolve_model_path(raw: str) -> str:
    """Resolve model path for faster-whisper.

    faster-whisper expects a directory containing CTranslate2 model files
    (e.g., model.bin, config.json). If the user points to a file inside
    the directory, use the parent directory instead.
    """
    import os

    path = raw
    # If it's a file (e.g. .../model.bin), use parent directory
    if os.path.isfile(path):
        path = os.path.dirname(path)

    return path


def _is_gpu_runtime_error(exc: Exception) -> bool:
    """Detects missing CUDA/cuDNN/cuBLAS runtime libraries."""
    msg = str(exc).lower()
    return any(
        token in msg
        for token in ("cublas", "cudnn", "cuda", "libcublas", "no cuda", "gpu")
    )


def _run(model_dir, audio_path, device, compute_type, language, task, threads):
    from faster_whisper import WhisperModel

    model = WhisperModel(
        model_dir,
        device=device,
        compute_type=compute_type,
        num_workers=threads,
        cpu_threads=threads,
    )
    segments_result, info = model.transcribe(
        audio_path,
        language=language,
        task=task,
        beam_size=5,
        vad_filter=True,
    )
    # Consume the generator here so GPU/encode errors surface inside this try.
    all_segments = list(segments_result)
    return all_segments, info


def transcribe(
    model_path: str,
    audio_path: str,
    device: str,
    compute_type: str,
    language: str | None,
    task: str,
    threads: int,
) -> dict:
    try:
        from faster_whisper import WhisperModel  # noqa: F401
    except ImportError:
        print(
            json.dumps({
                "error": "faster-whisper not installed. Run: pip install faster-whisper"
            })
        )
        sys.exit(1)

    if language == "" or language is None:
        language = None

    model_dir = resolve_model_path(model_path)
    start = time.monotonic()

    try:
        all_segments, info = _run(
            model_dir, audio_path, device, compute_type, language, task, threads
        )
    except Exception as exc:  # noqa: BLE001
        # If the GPU runtime is unavailable (e.g. cublas64_12.dll missing) and we
        # were not already on CPU, retry transparently on CPU.
        if device != "cpu" and _is_gpu_runtime_error(exc):
            print(
                f"[transcrever] GPU indisponivel ({exc}); refazendo na CPU.",
                file=sys.stderr,
            )
            all_segments, info = _run(
                model_dir, audio_path, "cpu", "auto", language, task, threads
            )
        else:
            raise

    elapsed = time.monotonic() - start

    segments = []
    raw_texts = []
    for idx, seg in enumerate(all_segments):
        segments.append({
            "segment_index": idx,
            "start_ms": round(seg.start * 1000),
            "end_ms": round(seg.end * 1000),
            "text": seg.text.strip(),
            "confidence": round(seg.avg_logprob, 4),
        })
        raw_texts.append(seg.text.strip())

    return {
        "raw_text": "\n".join(raw_texts),
        "segments": segments,
        "detected_language": info.language,
        "duration_ms": round(info.duration * 1000),
        "elapsed_ms": round(elapsed * 1000),
    }


def main():
    parser = argparse.ArgumentParser(description="faster-whisper transcription")
    parser.add_argument("--model", required=True, help="Model directory path")
    parser.add_argument("--audio", required=True, help="Audio file path")
    parser.add_argument("--device", default="cpu", choices=["cpu", "cuda", "auto"])
    parser.add_argument("--compute-type", default="auto",
                        choices=["auto", "float16", "int8_float16", "int8", "int8_bfloat16"])
    parser.add_argument("--language", default=None, help="Language code (pt, en, etc.)")
    parser.add_argument("--task", default="transcribe", choices=["transcribe", "translate"])
    parser.add_argument("--threads", type=int, default=4)

    args = parser.parse_args()

    result = transcribe(
        model_path=args.model,
        audio_path=args.audio,
        device=args.device,
        compute_type=args.compute_type,
        language=args.language,
        task=args.task,
        threads=args.threads,
    )

    json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
    print()


if __name__ == "__main__":
    main()
