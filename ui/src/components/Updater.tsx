import React, { useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { DownloadSimple } from "@phosphor-icons/react";

type Update = { state: string; version: string | null; reason: string };

/** A quiet header action; inaccessible private feeds never interrupt Access. */
export function Updater() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!isTauri()) return;
    let alive = true;
    const refresh = () => {
      invoke<Update>("update_status")
        .then((value) => {
          if (alive) setUpdate(value);
        })
        .catch(() => {});
    };
    refresh();
    const timer = window.setInterval(refresh, 10000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);
  if (update?.state !== "ready") return null;
  const title =
    error ||
    `Install Driftnet ${update.version}. The system installer will open.`;
  return (
    <button
      className="icon-button"
      title={title}
      aria-label={title}
      disabled={busy}
      onClick={async () => {
        setBusy(true);
        setError("");
        try {
          await invoke("install_update");
        } catch (cause) {
          setError(String(cause));
          setBusy(false);
        }
      }}
    >
      <DownloadSimple size={19} weight="bold" />
    </button>
  );
}
