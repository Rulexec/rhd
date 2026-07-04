import { writable, derived } from 'svelte/store';
import type { Writable, Readable } from 'svelte/store';
import type { ActiveScenario, FinishedScenario, PausedScenario } from './types/index';

interface ActiveScenarioStore extends Readable<Map<string, ActiveScenario>> {
  setFromList(list: Array<{ id: string; scenarioName: string; startedAt: string }>): void;
  addScenario(data: { id: string; name: string; startedAt: string }): void;
  updateStep(executionId: string, stepName: string, stepStartedAt: string): void;
  removeScenario(id: string): void;
}

function createActiveScenariosStore(): ActiveScenarioStore {
  const { subscribe, update, set } = writable(new Map<string, ActiveScenario>());

  return {
    subscribe,
    setFromList(list) {
      set(new Map(list.map((item) => [String(item.id), {
        id: String(item.id),
        scenarioName: item.scenarioName,
        startedAt: item.startedAt,
        currentStep: null,
        currentStepStartedAt: null,
        promptTokens: 0,
        completionTokens: 0,
      }])));
    },
    addScenario(data) {
      update((map) => {
        const next = new Map(map);
        next.set(data.id, {
          id: data.id,
          scenarioName: data.name,
          startedAt: data.startedAt,
          currentStep: null,
          currentStepStartedAt: null,
          promptTokens: 0,
          completionTokens: 0,
        });
        return next;
      });
    },
    updateStep(executionId, stepName, stepStartedAt) {
      update((map) => {
        const next = new Map(map);
        const scenario = next.get(executionId);
        if (scenario) {
          next.set(executionId, { ...scenario, currentStep: stepName, currentStepStartedAt: stepStartedAt });
        }
        return next;
      });
    },
    removeScenario(id) {
      update((map) => {
        const next = new Map(map);
        next.delete(id);
        return next;
      });
    },
  };
}

interface FinishedScenariosStore extends Readable<FinishedScenario[]> {
  setAll(list: FinishedScenario[]): void;
  prepend(scenarioMeta: FinishedScenario): void;
}

function createFinishedScenariosStore(): FinishedScenariosStore {
  const { subscribe, update, set } = writable<FinishedScenario[]>([]);

  return {
    subscribe,
    setAll(list) {
      set(list);
    },
    prepend(scenarioMeta) {
      update((list) => [scenarioMeta, ...list]);
    },
  };
}

interface PausedScenariosStore extends Readable<Map<string, PausedScenario>> {
  addScenario(data: PausedScenario): void;
  removeScenario(executionId: string): void;
  updateSelectedModel(executionId: string, model: string | null): void;
}

function createPausedScenariosStore(): PausedScenariosStore {
  const { subscribe, update, set } = writable(new Map<string, PausedScenario>());

  return {
    subscribe,
    addScenario(data) {
      update((map) => {
        const next = new Map(map);
        next.set(data.executionId, data);
        return next;
      });
    },
    removeScenario(executionId) {
      update((map) => {
        const next = new Map(map);
        next.delete(executionId);
        return next;
      });
    },
    updateSelectedModel(executionId, model) {
      update((map) => {
        const next = new Map(map);
        const scenario = next.get(executionId);
        if (scenario) {
          next.set(executionId, { ...scenario, selectedModel: model });
        }
        return next;
      });
    },
  };
}

export const activeScenarios = createActiveScenariosStore();
export const finishedScenarios = createFinishedScenariosStore();
export const pausedScenarios = createPausedScenariosStore();
export const lastKnownId: Writable<number> = writable(0);
export const wsConnected: Writable<boolean> = writable(false);

export const activeScenariosList: Readable<ActiveScenario[]> = derived(activeScenarios, ($map) =>
  [...$map.values()].sort((a, b) => b.id.localeCompare(a.id))
);
