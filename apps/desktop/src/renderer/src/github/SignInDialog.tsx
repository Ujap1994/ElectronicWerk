import { useEffect, useRef, useState, type ReactNode } from 'react';

import type { DeviceStart } from '@shared/github';

import { Icon } from '../components/Icon';
import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { describeGitError, githubStatus, useGithubStatus } from './stores';

type DeviceState =
  | { readonly state: 'idle' }
  | { readonly state: 'starting' }
  | { readonly state: 'waiting'; readonly start: DeviceStart }
  | { readonly state: 'ended'; readonly message: string };

/**
 * Sign-in to GitHub: the device flow (the user confirms a code on
 * github.com) or a personal access token.
 */
export function SignInDialog({ onClose }: { readonly onClose: () => void }): ReactNode {
  const { t } = useAppState();
  const { notify } = useProject();
  const status = useGithubStatus();
  const [device, setDevice] = useState<DeviceState>({ state: 'idle' });
  const [copied, setCopied] = useState(false);
  const [token, setToken] = useState('');
  const [tokenError, setTokenError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const mountedRef = useRef(true);
  const isOpen = (): boolean => mountedRef.current;

  useEffect(
    () => () => {
      mountedRef.current = false;
      window.ew.github.deviceCancel();
    },
    [],
  );

  const signedIn = (login: string): void => {
    notify(t('github.signIn.done', { login }));
    onClose();
  };

  const startDevice = (): void => {
    setDevice({ state: 'starting' });
    void window.ew.github.deviceStart().then(async (result) => {
      if (!isOpen()) {
        return;
      }
      if (!result.ok) {
        setDevice({ state: 'ended', message: describeGitError(t, result.error) });
        return;
      }
      setDevice({ state: 'waiting', start: result.value });
      const end = await window.ew.github.deviceWait();
      if (!isOpen()) {
        return;
      }
      switch (end.state) {
        case 'done':
          githubStatus.set({ ...(githubStatus.get() ?? fallbackStatus()), account: end.account });
          signedIn(end.account.login);
          return;
        case 'denied':
          setDevice({ state: 'ended', message: t('github.signIn.denied') });
          return;
        case 'expired':
          setDevice({ state: 'ended', message: t('github.signIn.expired') });
          return;
        case 'failed':
          setDevice({
            state: 'ended',
            message: `${t('github.errors.failed')} ${t('github.errors.details', { message: end.message ?? '' })}`,
          });
      }
    });
  };

  const signInWithToken = (): void => {
    if (token.trim() === '') {
      return;
    }
    setChecking(true);
    setTokenError(null);
    void window.ew.github.signInWithToken(token).then((result) => {
      if (!isOpen()) {
        return;
      }
      setChecking(false);
      if (!result.ok) {
        setTokenError(describeGitError(t, result.error));
        return;
      }
      githubStatus.set({ ...(githubStatus.get() ?? fallbackStatus()), account: result.value });
      signedIn(result.value.login);
    });
  };

  return (
    <Modal title={t('github.signIn.title')} onClose={onClose} testId="github-sign-in">
      <p className="dialog__text">{t('github.signIn.intro')}</p>

      {status?.deviceFlow === true && (
        <fieldset className="field field--group" data-testid="github-device">
          <legend className="field__label">{t('github.signIn.device')}</legend>
          <span className="field__hint">{t('github.signIn.deviceHint')}</span>
          {(device.state === 'idle' || device.state === 'ended') && (
            <div>
              <button
                type="button"
                className="button button--primary"
                data-testid="github-device-start"
                onClick={startDevice}
              >
                <Icon name="github" size={16} />
                {t('github.signIn.deviceStart')}
              </button>
            </div>
          )}
          {device.state === 'starting' && (
            <span className="field__hint">{t('github.signIn.waiting')}</span>
          )}
          {device.state === 'waiting' && (
            <div className="github-device">
              <span>{t('github.signIn.deviceCode')}</span>
              <code className="github-device__code" data-testid="github-device-code">
                {device.start.userCode}
              </code>
              <div className="field__row">
                <button
                  type="button"
                  className="button"
                  data-testid="github-device-copy"
                  onClick={() => {
                    void navigator.clipboard.writeText(device.start.userCode).then(() => {
                      setCopied(true);
                    });
                  }}
                >
                  <Icon name="copy" size={16} />
                  {copied ? t('github.signIn.copied') : t('github.signIn.copy')}
                </button>
                <button
                  type="button"
                  className="button button--primary"
                  data-testid="github-device-open"
                  onClick={() => {
                    window.ew.github.openPage('device');
                  }}
                >
                  <Icon name="github" size={16} />
                  {t('github.signIn.openGithub')}
                </button>
              </div>
              <span className="field__hint" role="status">
                {t('github.signIn.waiting')}
              </span>
            </div>
          )}
          {device.state === 'ended' && (
            <span className="field__error" role="alert" data-testid="github-device-error">
              {device.message}
            </span>
          )}
        </fieldset>
      )}

      <fieldset className="field field--group" data-testid="github-token">
        <legend className="field__label">{t('github.signIn.token')}</legend>
        <span className="field__hint">{t('github.signIn.tokenHint')}</span>
        <div>
          <button
            type="button"
            className="button button--small"
            data-testid="github-token-create"
            onClick={() => {
              window.ew.github.openPage('tokens');
            }}
          >
            {t('github.signIn.tokenCreate')}
          </button>
        </div>
        <label className="field">
          <span className="field__label">{t('github.signIn.tokenLabel')}</span>
          <div className="field__row">
            <input
              className="field__control field__control--grow"
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={token}
              placeholder={t('github.signIn.tokenPlaceholder')}
              data-testid="github-token-input"
              onChange={(event) => {
                setToken(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  signInWithToken();
                }
              }}
            />
            <button
              type="button"
              className="button button--primary"
              disabled={token.trim() === '' || checking}
              data-testid="github-token-sign-in"
              onClick={signInWithToken}
            >
              {t('github.signIn.tokenSignIn')}
            </button>
          </div>
        </label>
        {tokenError !== null && (
          <span className="field__error" role="alert" data-testid="github-token-error">
            {tokenError}
          </span>
        )}
      </fieldset>
    </Modal>
  );
}

function fallbackStatus(): NonNullable<ReturnType<typeof githubStatus.get>> {
  return { account: null, deviceFlow: false, storage: 'keychain' };
}
