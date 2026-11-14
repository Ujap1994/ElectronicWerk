/** Shows the GitHub dialogs and keeps the account up to date. */
import { useEffect, useSyncExternalStore, type ReactNode } from 'react';

import { CloneDialog } from './CloneDialog';
import { SignInDialog } from './SignInDialog';
import { cloneOpen, githubStatus, signInOpen } from './stores';

export function GithubHost(): ReactNode {
  const signIn = useSyncExternalStore(signInOpen.subscribe, signInOpen.get);
  const clone = useSyncExternalStore(cloneOpen.subscribe, cloneOpen.get);

  useEffect(
    () =>
      window.ew.github.onChanged((status) => {
        githubStatus.set(status);
      }),
    [],
  );

  return (
    <>
      {clone && (
        <CloneDialog
          onClose={() => {
            cloneOpen.set(false);
          }}
        />
      )}
      {signIn && (
        <SignInDialog
          onClose={() => {
            signInOpen.set(false);
          }}
        />
      )}
    </>
  );
}
