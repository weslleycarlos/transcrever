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
