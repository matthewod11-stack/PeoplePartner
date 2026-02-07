import { useEffect, useState } from 'react';
import { check, Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export function useUpdateCheck() {
  const [updateAvailable, setUpdateAvailable] = useState<Update | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    checkForUpdate();
  }, []);

  async function checkForUpdate() {
    setChecking(true);
    setError(null);
    try {
      const update = await check();
      if (update) {
        setUpdateAvailable(update);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to check for updates');
    } finally {
      setChecking(false);
    }
  }

  async function installUpdate() {
    if (!updateAvailable) return;
    setInstalling(true);
    try {
      await updateAvailable.downloadAndInstall();
      await relaunch();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to install update');
      setInstalling(false);
    }
  }

  return { updateAvailable, checking, installing, error, checkForUpdate, installUpdate };
}
