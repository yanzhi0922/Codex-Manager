import { invoke, withAddr } from "./transport";

export type SessionSelectionInput = {
  filePaths?: string[];
  ids?: string[];
  provider?: string;
  query?: string;
  limit?: number;
  allowAll?: boolean;
};

export const sessionClient = {
  async scanSessions(params?: {
    sessionsDir?: string;
    page?: number;
    pageSize?: number;
    query?: string;
    provider?: string;
    includePreview?: boolean;
  }): Promise<unknown> {
    return invoke("service_session_scan", withAddr(params ?? {}));
  },

  async getOverview(params?: { sessionsDir?: string }): Promise<unknown> {
    return invoke("service_session_overview", withAddr(params ?? {}));
  },

  async getSessionDetail(params: {
    path: string;
    sessionsDir?: string;
  }): Promise<unknown> {
    return invoke("service_session_detail", withAddr(params));
  },

  async getDashboard(params?: {
    sessionsDir?: string;
    page?: number;
    pageSize?: number;
    query?: string;
    provider?: string;
    includePreview?: boolean;
  }): Promise<unknown> {
    return invoke("service_session_dashboard", withAddr(params ?? {}));
  },

  async runDoctor(params?: { sessionsDir?: string }): Promise<unknown> {
    return invoke("service_session_doctor", withAddr(params ?? {}));
  },

  async previewMigration(params: {
    sessionsDir?: string;
    selection: SessionSelectionInput;
    targetProvider: string;
    targetSource?: string;
  }): Promise<unknown> {
    return invoke("service_session_migrate_preview", withAddr(params));
  },

  async migrateSessions(params: {
    sessionsDir?: string;
    selection: SessionSelectionInput;
    targetProvider: string;
    targetSource?: string;
    dryRun?: boolean;
  }): Promise<unknown> {
    return invoke("service_session_migrate", withAddr(params));
  },

  async exportSessions(params: {
    sessionsDir?: string;
    selection: SessionSelectionInput;
    format: string;
    filePrefix?: string;
  }): Promise<unknown> {
    return invoke("service_session_export", withAddr(params));
  },

  async repairIndex(params?: { sessionsDir?: string }): Promise<unknown> {
    return invoke("service_session_repair", withAddr(params ?? {}));
  },

  async listBackups(params?: { sessionsDir?: string }): Promise<unknown> {
    return invoke("service_session_backups", withAddr(params ?? {}));
  },
};
