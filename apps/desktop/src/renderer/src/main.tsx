import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { ProjectProvider } from './project/ProjectProvider';
import { AppStateProvider } from './state';
import { View3dWindow, viewFromHash } from './view3d/View3dWindow';
import { installWindowSync } from './window-sync';
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
  installWindowSync();
  // A view window (3D view on a second monitor) or the main window.
  const view = viewFromHash(window.location.hash);
  createRoot(container).render(
    <StrictMode>
      <AppStateProvider initial={{ appInfo, snapshot, engineStatus }}>
        {view === null ? (
          <ProjectProvider>
            <App />
          </ProjectProvider>
        ) : (
          <View3dWindow doc={view.doc} name={view.name} />
        )}
      </AppStateProvider>
    </StrictMode>,
  );
}

void bootstrap();
