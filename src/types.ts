export type JobStatus = "pending" | "processing" | "completed" | "error" | "reviewed" | "exported";

export interface MediaFile {
  id: number;
  relativePath: string;
  fileName: string;
  extension: string;
  sizeBytes: number;
}

export interface JobSummary {
  id: number;
  mediaFileId: number;
  status: JobStatus;
  progress: number;
  errorMessage?: string | null;
}

export interface ScanResponse {
  discoveredCount: number;
  queuedCount: number;
}

export interface Segment {
  id: number;
  startMs: number;
  endMs: number;
  rawText: string;
  editedText?: string | null;
}

export interface ReviewDocument {
  mediaPath: string;
  rawText: string;
  editedText?: string | null;
  segments: Segment[];
}

export interface ProfileRow {
  id: number;
  name: string;
  backend: string;
  modelPath: string;
  device: string;
  precision: string;
  threads: number;
  language?: string | null;
  task: string;
  advancedJson: string;
}

export interface JobRow {
  jobId: number;
  mediaFileId: number;
  fileName: string;
  relativePath: string;
  status: string;
  progress: number;
  errorMessage?: string | null;
}

export interface SegmentView {
  id: number;
  segmentIndex: number;
  startMs: number;
  endMs: number;
  rawText: string;
  editedText?: string | null;
  confidence?: number | null;
}

export interface TranscriptionView {
  transcriptionId: number;
  mediaFileId: number;
  jobId: number;
  fileName: string;
  absolutePath: string;
  relativePath: string;
  extension: string;
  sizeBytes: number;
  durationMs?: number | null;
  modifiedAt: string;
  createdAt?: string | null;
  rawText: string;
  editedText?: string | null;
  segments: SegmentView[];
}
