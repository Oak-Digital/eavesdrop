import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "./api";

export type UpdatePhase = "idle" | "checking" | "up_to_date" | "available" | "downloading" | "error";

export interface UpdateState {
  phase: UpdatePhase;
  currentVersion: string;
  availableVersion: string | null;
  progress: number | null;
  error: string | null;
}

const initialState: UpdateState = {
  phase: "idle",
  currentVersion: "",
  availableVersion: null,
  progress: null,
  error: null,
};

export function downloadPercentage(downloaded: number, total?: number): number | null {
  if (!total || total <= 0) return null;
  return Math.min(100, Math.max(0, Math.round((downloaded / total) * 100)));
}

export function useUpdater(enabled: boolean) {
  const [state, setState] = useState<UpdateState>(initialState);
  const updateRef = useRef<Update | null>(null);
  const downloadedRef = useRef(false);
  const busyRef = useRef(false);
  const isDesktop = "__TAURI_INTERNALS__" in window;

  const checkForUpdates = useCallback(async () => {
    if (!enabled || !isDesktop || busyRef.current) return;
    busyRef.current = true;
    setState((current) => ({ ...current, phase: "checking", error: null, progress: null }));
    try {
      const update = await check({ timeout: 15_000 });
      if (updateRef.current && updateRef.current !== update) await updateRef.current.close();
      updateRef.current = update;
      downloadedRef.current = false;
      setState((current) => update ? {
        ...current,
        phase: "available",
        currentVersion: update.currentVersion,
        availableVersion: update.version,
        error: null,
      } : {
        ...current,
        phase: "up_to_date",
        availableVersion: null,
        error: null,
      });
    } catch (cause) {
      setState((current) => ({
        ...current,
        phase: "error",
        error: cause instanceof Error ? cause.message : String(cause),
      }));
    } finally {
      busyRef.current = false;
    }
  }, [enabled, isDesktop]);

  const installUpdate = useCallback(async () => {
    const update = updateRef.current;
    if (!update || busyRef.current) return;
    busyRef.current = true;
    let downloaded = 0;
    let total: number | undefined;
    setState((current) => ({ ...current, phase: "downloading", progress: downloadedRef.current ? 100 : 0, error: null }));
    try {
      if (!downloadedRef.current) {
        await update.download((event) => {
          if (event.event === "Started") {
            total = event.data.contentLength;
            downloaded = 0;
          } else if (event.event === "Progress") {
            downloaded += event.data.chunkLength;
          }
          setState((current) => ({ ...current, progress: event.event === "Finished" ? 100 : downloadPercentage(downloaded, total) }));
        }, { timeout: 120_000 });
        downloadedRef.current = true;
      }
      await api.beginUpdateInstall();
      await update.install();
      await relaunch();
    } catch (cause) {
      try { await api.cancelUpdateInstall(); } catch { /* Relaunch safety lock clears on process exit. */ }
      setState((current) => ({
        ...current,
        phase: downloadedRef.current ? "available" : "error",
        progress: downloadedRef.current ? 100 : null,
        error: cause instanceof Error ? cause.message : String(cause),
      }));
      busyRef.current = false;
    }
  }, []);

  useEffect(() => {
    if (!enabled || !isDesktop) return;
    void getVersion()
      .then((currentVersion) => setState((current) => ({ ...current, currentVersion })))
      .catch(() => undefined);
    const timer = window.setTimeout(() => { void checkForUpdates(); }, 3_000);
    return () => window.clearTimeout(timer);
  }, [checkForUpdates, enabled, isDesktop]);

  useEffect(() => () => {
    if (updateRef.current) void updateRef.current.close();
  }, []);

  return { state, checkForUpdates, installUpdate };
}
