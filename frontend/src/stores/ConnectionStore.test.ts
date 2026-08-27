import { describe, it, expect } from 'vitest';
import { ConnectionStore } from './ConnectionStore.js';

describe('ConnectionStore', () => {
  it('should initialize with disconnected status', () => {
    const store = new ConnectionStore();
    expect(store.status).toBe('disconnected');
    expect(store.error).toBe(null);
    expect(store.isConnected).toBe(false);
  });

  it('should set connecting status', () => {
    const store = new ConnectionStore();
    store.setConnecting();
    expect(store.status).toBe('connecting');
    expect(store.error).toBe(null);
    expect(store.isConnected).toBe(false);
  });

  it('should set connected status', () => {
    const store = new ConnectionStore();
    store.setConnected();
    expect(store.status).toBe('connected');
    expect(store.error).toBe(null);
    expect(store.isConnected).toBe(true);
  });

  it('should set disconnected status with error', () => {
    const store = new ConnectionStore();
    store.setDisconnected('Connection lost');
    expect(store.status).toBe('disconnected');
    expect(store.error).toBe('Connection lost');
    expect(store.isConnected).toBe(false);
  });

  it('should clear error', () => {
    const store = new ConnectionStore();
    store.setDisconnected('Some error');
    expect(store.error).toBe('Some error');

    store.clearError();
    expect(store.error).toBe(null);
    expect(store.status).toBe('disconnected');
  });
});