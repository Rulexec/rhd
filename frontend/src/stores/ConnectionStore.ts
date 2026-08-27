import { makeAutoObservable } from 'mobx';

export type ConnectionStatus = 'connected' | 'disconnected' | 'connecting';

export interface ConnectionState {
  status: ConnectionStatus;
  error: string | null;
}

/**
 * MobX store for WebSocket connection state.
 */
export class ConnectionStore {
  status: ConnectionStatus = 'disconnected';
  error: string | null = null;

  constructor() {
    makeAutoObservable(this);
  }

  /**
   * Set connection status to connecting.
   */
  setConnecting(): void {
    this.status = 'connecting';
    this.error = null;
  }

  /**
   * Set connection status to connected.
   */
  setConnected(): void {
    this.status = 'connected';
    this.error = null;
  }

  /**
   * Set connection status to disconnected with optional error.
   */
  setDisconnected(error: string | null = null): void {
    this.status = 'disconnected';
    this.error = error;
  }

  /**
   * Check if currently connected.
   */
  get isConnected(): boolean {
    return this.status === 'connected';
  }

  /**
   * Clear any error state.
   */
  clearError(): void {
    this.error = null;
  }
}