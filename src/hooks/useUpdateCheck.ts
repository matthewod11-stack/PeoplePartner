import { useEffect, useState } from 'react';
import { check, Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export type UpdatePhase = 'idle' | 'checking' | 'downloading' | 'relaunching';

const SNOOZE_STORAGE_KEY = 'peoplepartner.updateSnooze';
const SNOOZE_DURATION_MS = 24 * 60 * 60 * 1000;

export interface SnoozeRecord {
  /** Update version this snooze applies to. A newer version invalidates it. */
  version: string;
  /** Wall-clock ms when the snooze expires. */
  untilMs: number;
}

/**
 * Pure snooze-evaluation. Exported so unit tests can drive it without a
 * real Tauri Update object or Date.now().
 */
export function isUpdateSnoozed(
  update: { version: string } | null,
  record: SnoozeRecord | null,
  nowMs: number
): boolean {
  if (!update || !record) return false;
  // A newer release invalidates an old snooze: customers shouldn't be
  // silently locked out of a security release because they tapped "Later"
  // on the prior one.
  if (record.version !== update.version) return false;
  return nowMs < record.untilMs;
}

function readSnooze(): SnoozeRecord | null {
  try {
    const raw = localStorage.getItem(SNOOZE_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<SnoozeRecord> | null;
    if (
      !parsed ||
      typeof parsed.version !== 'string' ||
      typeof parsed.untilMs !== 'number'
    ) {
      return null;
    }
    return { version: parsed.version, untilMs: parsed.untilMs };
  } catch {
    return null;
  }
}

function writeSnooze(record: SnoozeRecord): void {
  try {
    localStorage.setItem(SNOOZE_STORAGE_KEY, JSON.stringify(record));
  } catch {
    // localStorage may be unavailable (private mode, quota); snooze just
    // won't persist across launches. Failing the click is worse UX.
  }
}

export function useUpdateCheck() {
  const [updateAvailable, setUpdateAvailable] = useState<Update | null>(null);
  const [phase, setPhase] = useState<UpdatePhase>('idle');
  const [error, setError] = useState<string | null>(null);
  const [snoozeRecord, setSnoozeRecord] = useState<SnoozeRecord | null>(() =>
    readSnooze()
  );

  useEffect(() => {
    void checkForUpdate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function checkForUpdate() {
    setPhase('checking');
    setError(null);
    try {
      const update = await check();
      if (update) {
        setUpdateAvailable(update);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to check for updates');
    } finally {
      setPhase('idle');
    }
  }

  async function installUpdate() {
    if (!updateAvailable) return;
    setError(null);
    setPhase('downloading');
    try {
      await updateAvailable.downloadAndInstall((progress) => {
        if (progress.event === 'Finished') {
          setPhase('relaunching');
        }
      });
      await relaunch();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to install update');
      setPhase('idle');
    }
  }

  /** Defer this version's update prompt for 24h. */
  function snoozeUpdate() {
    if (!updateAvailable) return;
    const record: SnoozeRecord = {
      version: updateAvailable.version,
      untilMs: Date.now() + SNOOZE_DURATION_MS,
    };
    writeSnooze(record);
    setSnoozeRecord(record);
  }

  async function retry() {
    setError(null);
    if (updateAvailable) {
      await installUpdate();
    } else {
      await checkForUpdate();
    }
  }

  const checking = phase === 'checking';
  const installing = phase === 'downloading' || phase === 'relaunching';
  const isSnoozed = isUpdateSnoozed(updateAvailable, snoozeRecord, Date.now());

  return {
    updateAvailable,
    isSnoozed,
    phase,
    checking,
    installing,
    error,
    checkForUpdate,
    installUpdate,
    snoozeUpdate,
    retry,
  };
}
