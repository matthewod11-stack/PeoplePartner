import { useEffect, useState } from 'react';
import { check, Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export type UpdatePhase = 'idle' | 'checking' | 'downloading' | 'relaunching';

export function useUpdateCheck() {
  const [updateAvailable, setUpdateAvailable] = useState<Update | null>(null);
  const [phase, setPhase] = useState<UpdatePhase>('idle');
  const [error, setError] = useState<string | null>(null);

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

  return {
    updateAvailable,
    phase,
    checking,
    installing,
    error,
    checkForUpdate,
    installUpdate,
    retry,
  };
}
