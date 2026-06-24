# Manual Test Checklist

## Setup

- Install frontend dependencies with `npm install`.
- Confirm Rust toolchain is installed with `cargo --version`.
- Place a short `.mp3` or `.wav` file in a sample folder.
- Configure a local `whisper.cpp` executable and compatible model before testing transcription.

## Smoke Test

1. Run `npm run tauri dev`.
2. Select the sample folder as origin.
3. Confirm the app reports discovered files.
4. Select an export folder.
5. Start one transcription job.
6. Confirm the job reaches completed or shows a clear error.
7. Open the review area.
8. Play audio.
9. Edit a segment.
10. Edit continuous text.
11. Export `.txt`.
12. Close and reopen the app.
13. Confirm queue and transcription state remain available.

## Failure Test

1. Configure an invalid model path.
2. Start a transcription.
3. Confirm the job enters error state.
4. Confirm the rest of the queue remains usable.
