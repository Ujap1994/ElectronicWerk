/**
 * The online part libraries: a search field that opens SnapEDA or Ultra
 * Librarian at their search page (in the provider window or the user's own
 * browser), and the import of a downloaded file. ElectronicWerk itself never
 * searches or downloads on the providers' websites.
 */
import { useEffect, useState, type ReactNode } from 'react';

import { PROVIDERS, type ProviderId, type ProviderStatus } from '@shared/providers';

import { Icon } from '../components/Icon';
import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { onlineParts, pendingImport } from './stores';

export function OnlineParts({ initialQuery }: { readonly initialQuery: string }): ReactNode {
  const { t } = useAppState();
  const { project } = useProject();
  const [query, setQuery] = useState(initialQuery);
  const [status, setStatus] = useState<readonly ProviderStatus[]>([]);

  useEffect(() => {
    let cancelled = false;
    void window.ew.providers.status().then((list) => {
      if (!cancelled) {
        setStatus(list);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const close = (): void => {
    onlineParts.set(null);
  };
  const search = (id: ProviderId, external: boolean): void => {
    const page = query.trim() === '' ? 'home' : 'search';
    if (external) {
      window.ew.providers.openExternal(id, page, query.trim());
    } else {
      window.ew.providers.open(id, page, query.trim());
    }
  };
  const importFile = (): void => {
    void window.ew.providers.chooseDownload().then((path) => {
      if (path !== null) {
        close();
        pendingImport.set(path);
      }
    });
  };

  return (
    <Modal
      title={t('parts.title')}
      onClose={close}
      testId="online-parts"
      className="dialog--wide"
      footer={
        <>
          <button
            type="button"
            className="button"
            title={
              project === null ? t('parts.noProject', { file: '…' }) : t('parts.importFileTip')
            }
            disabled={project === null}
            data-testid="online-import-file"
            onClick={importFile}
          >
            <Icon name="importFile" />
            <span>{t('parts.importFile')}</span>
          </button>
          <button type="button" className="button button--primary" onClick={close}>
            {t('settings.close')}
          </button>
        </>
      }
    >
      <p className="dialog__text">{t('parts.intro')}</p>
      <label className="field">
        <span className="field__label">{t('parts.query')}</span>
        <input
          className="field__control"
          type="search"
          value={query}
          placeholder={t('parts.queryPlaceholder')}
          data-testid="online-query"
          // The dialog opens to type a part number.
          autoFocus
          onChange={(event) => {
            setQuery(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              search('snapeda', false);
            }
          }}
        />
      </label>
      <div className="online-parts__providers">
        {PROVIDERS.map((provider) => {
          const signedIn = status.find((s) => s.id === provider.id)?.session ?? false;
          return (
            <section
              key={provider.id}
              className="online-parts__provider"
              data-testid={`online-provider-${provider.id}`}
            >
              <h3 className="online-parts__name">{provider.name}</h3>
              <p className="online-parts__text">{t(`parts.providers.${provider.id}`)}</p>
              <p className="online-parts__status" data-session={signedIn}>
                {signedIn ? t('parts.session') : t('parts.noSession')}
              </p>
              <div className="online-parts__actions">
                <button
                  type="button"
                  className="button button--primary"
                  data-testid={`online-search-${provider.id}`}
                  onClick={() => {
                    search(provider.id, false);
                  }}
                >
                  <Icon name="search" />
                  <span>{t('parts.search', { provider: provider.name })}</span>
                </button>
                <button
                  type="button"
                  className="button"
                  title={t('parts.searchExternalTip', { provider: provider.name })}
                  data-testid={`online-external-${provider.id}`}
                  onClick={() => {
                    search(provider.id, true);
                  }}
                >
                  <Icon name="reveal" />
                  <span>{t('parts.searchExternal')}</span>
                </button>
              </div>
            </section>
          );
        })}
      </div>
      <p className="field__hint">{t('parts.howItWorks')}</p>
    </Modal>
  );
}

/** The online part libraries in the settings: sign in, sign out, terms. */
export function ProviderSettings(): ReactNode {
  const { t } = useAppState();
  const [status, setStatus] = useState<readonly ProviderStatus[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [version, setVersion] = useState(0);

  useEffect(() => {
    let cancelled = false;
    void window.ew.providers.status().then((list) => {
      if (!cancelled) {
        setStatus(list);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [version]);

  return (
    <fieldset className="field field--group" data-testid="settings-providers">
      <legend className="field__label">{t('parts.settings.title')}</legend>
      {PROVIDERS.map((provider) => {
        const signedIn = status.find((s) => s.id === provider.id)?.session ?? false;
        return (
          <div key={provider.id} className="field__row online-parts__setting">
            <span className="online-parts__setting-name">
              <strong>{provider.name}</strong>
              <span className="field__hint">
                {signedIn ? t('parts.session') : t('parts.noSession')}
              </span>
            </span>
            <button
              type="button"
              className="button button--small"
              data-testid={`provider-sign-in-${provider.id}`}
              onClick={() => {
                window.ew.providers.open(provider.id, 'signIn');
              }}
            >
              {t('parts.settings.signIn')}
            </button>
            <button
              type="button"
              className="button button--small"
              title={t('parts.settings.signOutTip')}
              disabled={!signedIn}
              data-testid={`provider-sign-out-${provider.id}`}
              onClick={() => {
                void window.ew.providers.signOut(provider.id).then(() => {
                  setMessage(t('parts.settings.signedOut', { provider: provider.name }));
                  setVersion((v) => v + 1);
                });
              }}
            >
              {t('parts.settings.signOut')}
            </button>
            <button
              type="button"
              className="button button--small"
              onClick={() => {
                window.ew.providers.openExternal(provider.id, 'terms');
              }}
            >
              {t('parts.settings.terms')}
            </button>
          </div>
        );
      })}
      {message !== null && <span className="field__hint">{message}</span>}
    </fieldset>
  );
}
