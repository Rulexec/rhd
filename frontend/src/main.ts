import { mount } from 'svelte';
import App from './App.svelte';
import { setupStateExportImport } from './lib/stateExport';
import './styles/global.css';
import './styles/utilities.css';

const app = mount(App, { target: document.getElementById('app')! });

setupStateExportImport();

export default app;
