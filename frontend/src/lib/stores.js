import { writable, derived } from 'svelte/store';

function createActiveScenariosStore() {
  const { subscribe, update, set } = writable(new Map());

  return {
    subscribe,
    setFromList(list) {
      set(new Map(list.map((item) => [item.id, {
        id: item.id,
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

function createFinishedScenariosStore() {
  const { subscribe, update, set } = writable([]);

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

export const activeScenarios = createActiveScenariosStore();
export const finishedScenarios = createFinishedScenariosStore();
export const lastKnownId = writable(0);
export const wsConnected = writable(false);

export const activeScenariosList = derived(activeScenarios, ($map) =>
  [...$map.values()].sort((a, b) => b.id - a.id)
);
