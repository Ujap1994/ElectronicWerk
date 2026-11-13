/**
 * Shows the dialogs of the online part libraries and offers every file
 * downloaded in a provider window for import.
 */
import { useEffect, useSyncExternalStore, type ReactNode } from 'react';

import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { DownloadImport } from './DownloadImport';
import { OnlineParts } from './OnlineParts';
import { onlineParts, pendingImport } from './stores';

export function PartsHost(): ReactNode {
  const { t } = useAppState();
  const { project, notify } = useProject();
  const online = useSyncExternalStore(onlineParts.subscribe, onlineParts.get);
  const pending = useSyncExternalStore(pendingImport.subscribe, pendingImport.get);
  const projectOpen = project !== null;

  useEffect(
    () =>
      window.ew.providers.onDownload((download) => {
        if (download.state === 'failed') {
          notify(t('parts.downloadFailed', { file: download.name }), 'error');
          return;
        }
        if (!download.importable) {
          const folder = download.path.slice(0, download.path.length - download.name.length);
          notify(t('parts.notImportable', { file: download.name, folder }));
          return;
        }
        if (!projectOpen) {
          notify(t('parts.noProject', { file: download.name }), 'error');
          return;
        }
        onlineParts.set(null);
        pendingImport.set(download.path);
      }),
    [notify, projectOpen, t],
  );

  return (
    <>
      {online !== null && pending === null && <OnlineParts initialQuery={online.query} />}
      {pending !== null && projectOpen && <DownloadImport key={pending} path={pending} />}
    </>
  );
}
