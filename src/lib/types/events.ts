export interface ModelStateEvent {
  event_type: string;
  model_id?: string;
  model_name?: string;
  error?: string;
}

export interface RecordingErrorEvent {
  error_type: string;
  detail?: string;
}

export interface LocalCleanupReviewRequestEvent {
  request_id: string;
  original_text: string;
  cleaned_text: string;
}
