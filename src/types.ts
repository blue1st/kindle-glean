export type ClippingType = 'Highlight' | 'Note' | 'Bookmark';

export interface Clipping {
  id: string;
  book_title: string;
  author?: string;
  clipping_type: ClippingType;
  location?: string;
  page?: number;
  created_at?: string;
  content: string;
}

export interface VocabLookup {
  id: string;
  word: string;
  stem?: string;
  lang?: string;
  usage?: string;
  book_title?: string;
  book_author?: string;
  timestamp: number;
}

export interface NotebookSummary {
  id: string;
  title: string;
  display_title: string;
  content_hash: string;
  last_modified: number;
  synced_at: string;
  relative_folder: string;
  cover_image_path?: string;
}

export interface SyncConfig {
  vault_path: string;
  sync_clippings: boolean;
  sync_vocab: boolean;
  sync_notebooks: boolean;
  auto_sync: boolean;
  auto_eject: boolean;
  subfolder: string;
  enable_push?: boolean;
  hotfolder_path?: string;
  hotfolder_dest_subfolder?: string;
  push_after_action?: "record_db" | "move_synced" | string;
  auto_convert_epub?: boolean;
}

export interface FileTransferResult {
  success: boolean;
  transferred_count: number;
  skipped_count: number;
  failed_files: [string, string][];
  message: string;
}

export interface HotfolderStatus {
  enabled: boolean;
  folder_path: string;
  exists: boolean;
  pending_files_count: number;
  pending_files: string[];
}

export interface SyncStats {
  highlights_added: number;
  notes_added: number;
  vocab_added: number;
  notebooks_added: number;
  total_books_updated: number;
  timestamp: string;
  device_name: string;
  success: boolean;
  message: string;
}

export interface DeviceInfo {
  device_id: string;
  device_type: string;
  connection_mode: string;
  mount_path: string;
  has_clippings: boolean;
  has_vocab: boolean;
  has_notebooks: boolean;
  connected: boolean;
  status_message?: string;
  is_registered: boolean;
  nickname?: string;
}

export interface DeviceProfile {
  device_id: string;
  nickname: string;
  device_type: string;
  vault_path: string;
  subfolder: string;
  sync_clippings: boolean;
  sync_vocab: boolean;
  sync_notebooks: boolean;
  auto_sync: boolean;
  auto_eject: boolean;
  created_at: string;
  last_connected_at: string;
}

export interface SyncProgress {
  step: string;
  message: string;
  percentage: number;
  current_item?: string;
}

export interface SyncedCounts {
  highlights: number;
  notes: number;
  vocab: number;
  notebooks: number;
}


