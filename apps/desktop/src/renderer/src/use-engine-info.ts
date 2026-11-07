import { useEffect, useState } from 'react';

import type { EngineInfo } from '@shared/engine';

import { useAppState } from './state';

const RETRY_DELAY_MS = 100;
const MAX_ATTEMPTS = 50;

/**
 * Asks the engine for its info over the MessagePort once per engine
 * generation. Returns `null` until the round trip succeeds.
 */
export function useEngineInfo(): EngineInfo | null {
  const { engine, engineStatus } = useAppState();
  // Remember which engine generation the info belongs to, so stale info from
  // a crashed engine is never shown.
  const [result, setResult] = useState<{ generation: number; info: EngineInfo } | null>(null);
  const running = engineStatus.state === 'running';
  const { generation } = engineStatus;

  useEffect(() => {
    if (!running) {
      return undefined;
    }
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const attempt = (count: number): void => {
      engine.call<EngineInfo>('engine.info').then(
        (info) => {
          if (!cancelled) {
            setResult({ generation, info });
          }
        },
        () => {
          // The port may not be attached yet right after a (re)start.
          if (!cancelled && count < MAX_ATTEMPTS) {
            timer = setTimeout(() => {
              attempt(count + 1);
            }, RETRY_DELAY_MS);
          }
        },
      );
    };
    attempt(1);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [engine, running, generation]);

  return running && result?.generation === generation ? result.info : null;
}
