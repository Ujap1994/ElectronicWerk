import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { ProjectProvider } from './project/ProjectProvider';
import { AppStateProvider } from './state';
import './styles.css';

async function bootstrap(): Promise<void> {
  const [appInfo, snapshot, engineStatus] = await Promise.all([
    window.ew.app.info(),
    window.ew.settings.get(),
    window.ew.engine.status(),
  ]);
  const container = document.getElementById('root');
  if (container === null) {
    throw new Error('missing #root element');
  }
  createRoot(container).render(
    <StrictMode>
      <AppStateProvider initial={{ appInfo, snapshot, engineStatus }}>
        <ProjectProvider>
          <App />
        </ProjectProvider>
      </AppStateProvider>
    </StrictMode>,
  );
}

void bootstrap();
