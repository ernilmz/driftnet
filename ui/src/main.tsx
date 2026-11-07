import React, { useEffect, useState, useRef } from "react";
import { createRoot } from "react-dom/client";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  GearSix,
  ArrowLeft,
  Plus,
  Trash,
  Globe,
  AppWindow,
  DiscordLogo,
  X,
  WarningCircle,
} from "@phosphor-icons/react";
import "@fontsource/ibm-plex-mono/400.css";
import { EdgeLight } from "./components/EdgeLight";
import { Updater } from "./components/Updater";
import "./style.css";
type Target =
  | { kind: "discord" }
  | { kind: "site"; domain: string; subdomains: boolean }
  | { kind: "app"; identity: string; display_name: string };
type Rule = { id: string; enabled: boolean; target: Target };
type Config = { schema_version: number; rules: Rule[] };
type Snapshot = {
  config: Config;
  available: boolean;
  state: string;
  reason: string;
  icons?: Record<string, string>;
};
const native = isTauri();
document.documentElement.classList.toggle("native", native);
const defaultConfig: Config = {
  schema_version: 1,
  rules: [
    {
      id: "00000000-0000-0000-0000-000000000001",
      enabled: true,
      target: { kind: "discord" },
    },
  ],
};
function previewConfig(): Config {
  try {
    const c = JSON.parse(localStorage.getItem("drift-preview-rules") || "null");
    return c?.schema_version === 1 && Array.isArray(c.rules)
      ? c
      : defaultConfig;
  } catch {
    return defaultConfig;
  }
}
function ruleName(rule: Rule) {
  return rule.target.kind === "discord"
    ? "Discord"
    : rule.target.kind === "site"
      ? rule.target.domain
      : rule.target.display_name;
}
function Toggle({
  on,
  disabled,
  onChange,
  label,
}: {
  on: boolean;
  disabled?: boolean;
  onChange: () => void;
  label: string;
}) {
  return (
    <button
      className="toggle"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      onClick={onChange}
    >
      <span />
    </button>
  );
}
function RuleIcon({ rule, icon }: { rule: Rule; icon?: string }) {
  const [failed, setFailed] = useState(false);
  const source =
    icon ||
    (rule.target.kind === "site"
      ? `https://${rule.target.domain}/favicon.ico`
      : undefined);
  useEffect(() => setFailed(false), [source]);
  if (source && !failed)
    return (
      <img
        className={"app-icon" + (rule.target.kind === "site" ? " favicon" : "")}
        src={source}
        alt=""
        loading="lazy"
        referrerPolicy="no-referrer"
        onError={() => setFailed(true)}
      />
    );
  return rule.target.kind === "discord" ? (
    <DiscordLogo size={21} weight="fill" />
  ) : rule.target.kind === "site" ? (
    <Globe size={20} />
  ) : (
    <AppWindow size={20} />
  );
}
function App() {
  useEffect(() => {
    if (native) invoke("panel_ready").catch(() => {});
  }, []);
  const [config, setConfig] = useState<Config>(
    native ? defaultConfig : previewConfig,
  );
  const [view, setView] = useState<"home" | "settings">("home");
  const [active, setActive] = useState(!native);
  const [busy, setBusy] = useState(false);
  const [ignition, setIgnition] = useState(false);
  const [error, setError] = useState("");
  const [site, setSite] = useState("");
  const [icons, setIcons] = useState<Record<string, string>>({});
  const [tab, setTab] = useState<"site" | "app">("site");
  const visibleRules = config.rules.filter((rule) =>
    tab === "site"
      ? rule.target.kind === "site" || rule.target.kind === "discord"
      : rule.target.kind === "app" || rule.target.kind === "discord",
  );
  const lastNetworkError = useRef("");
  const apply = (s: Snapshot) => {
    setConfig(s.config);
    setIcons(s.icons || {});
    setActive(s.state === "active");
    if (
      s.state === "error" &&
      s.reason &&
      lastNetworkError.current !== s.reason
    ) {
      lastNetworkError.current = s.reason;
      setError(s.reason);
    }
    if (s.state === "active" || s.state === "off")
      lastNetworkError.current = "";
  };
  useEffect(() => {
    if (native)
      invoke<Snapshot>("read_state")
        .then(apply)
        .catch((e) => setError(String(e)));
  }, []);
  useEffect(() => {
    if (!native)
      localStorage.setItem("drift-preview-rules", JSON.stringify(config));
  }, [config]);
  useEffect(() => {
    if (!native || busy) return;
    const timer = setInterval(() => {
      invoke<Snapshot>("read_state")
        .then(apply)
        .catch(() => {});
    }, 2000);
    return () => clearInterval(timer);
  }, [busy]);
  const run = async (f: () => Promise<void>) => {
    setError("");
    setBusy(true);
    try {
      await f();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const toggleAccess = () =>
    run(async () => {
      if (!active) {
        setIgnition(true);
        setTimeout(() => setIgnition(false), 1500);
      }
      if (native) {
        apply(await invoke<Snapshot>("set_access", { enabled: !active }));
      } else {
        setActive(!active);
      }
    });
  const addSite = () =>
    run(async () => {
      if (native) {
        apply(await invoke<Snapshot>("add_site", { input: site }));
      } else {
        let host: string;
        try {
          const u = new URL(
            site.includes("://") ? site : "https://" + site.trim(),
          );
          if (
            !["http:", "https:"].includes(u.protocol) ||
            u.username ||
            u.password
          )
            throw 0;
          host = u.hostname.toLowerCase().replace(/\.$/, "");
          if (
            !/^[a-z0-9-]+(?:\.[a-z0-9-]+)+$/.test(host) ||
            /^\d+(\.\d+)+$/.test(host) ||
            host.includes("*")
          )
            throw 0;
        } catch {
          throw "Enter a valid website, such as example.com.";
        }
        if (
          config.rules.some(
            (r) => r.target.kind === "site" && r.target.domain === host,
          )
        )
          throw "This item is already in your list.";
        setConfig({
          ...config,
          rules: [
            ...config.rules,
            {
              id: crypto.randomUUID(),
              enabled: true,
              target: { kind: "site", domain: host, subdomains: true },
            },
          ],
        });
      }
      setSite("");
    });
  const changeRule = (rule: Rule) =>
    run(async () => {
      if (native)
        apply(
          await invoke<Snapshot>("change_rule", {
            id: rule.id,
            enabled: !rule.enabled,
          }),
        );
      else
        setConfig({
          ...config,
          rules: config.rules.map((r) =>
            r.id === rule.id ? { ...r, enabled: !r.enabled } : r,
          ),
        });
    });
  const remove = (rule: Rule) =>
    run(async () => {
      if (native) apply(await invoke<Snapshot>("remove_rule", { id: rule.id }));
      else
        setConfig({
          ...config,
          rules: config.rules.filter((r) => r.id !== rule.id),
        });
    });
  const pickApp = () =>
    run(async () => {
      if (!native) {
        throw "The app picker is available in the Driftnet desktop app.";
      }
      const result = await invoke<Snapshot | null>("pick_app");
      if (result) apply(result);
    });
  useEffect(() => {
    if (native)
      invoke("set_panel_view", { settings: view === "settings" }).catch((e) =>
        setError(String(e)),
      );
  }, [view]);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (view === "settings") {
          setView("home");
          setError("");
        } else if (native) {
          invoke("hide_panel").catch((e) => setError(String(e)));
        }
      }
    };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, [view]);
  const pointerLight = (event: React.PointerEvent<HTMLElement>) => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const rect = event.currentTarget.getBoundingClientRect();
    event.currentTarget.style.setProperty(
      "--light-x",
      `${(event.clientX - rect.left - rect.width / 2) / 60}px`,
    );
    event.currentTarget.style.setProperty(
      "--light-y",
      `${(event.clientY - rect.top - rect.height / 2) / 60}px`,
    );
  };
  return (
    <main
      onPointerMove={pointerLight}
      onPointerLeave={(e) => {
        e.currentTarget.style.setProperty("--light-x", "0px");
        e.currentTarget.style.setProperty("--light-y", "0px");
      }}
      className={
        "panel " +
        (active ? "is-on" : "is-off") +
        (view === "settings" ? " settings-panel" : " home-panel") +
        (busy ? " is-busy" : "") +
        (ignition ? " ignition" : "")
      }
    >
      <div className="preview-note">
        {native ? "" : "Design preview · Network unchanged"}
      </div>
      <EdgeLight active={active} fast={busy || ignition} />
      <img
        className="surface-light"
        src="/assets/amber-surface.png"
        alt=""
        aria-hidden="true"
      />
      {view === "home" ? (
        <>
          <header data-tauri-drag-region>
            <h1 className="wordmark" aria-label="Driftnet">
              driftnet<span className="cursor">_</span>
            </h1>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <Updater />
              <button
                className="icon-button"
                aria-label="Settings"
                onClick={() => {
                  setView("settings");
                  setError("");
                }}
              >
                <GearSix size={19} weight="regular" />
              </button>
            </div>
          </header>
          <section className="access-row">
            <div>
              <h2>Access</h2>
              <p className="connection" aria-live="polite">
                <span className="status-dot" />
                {busy ? "Connecting" : active ? "On" : "Off"}
              </p>
            </div>
            <Toggle
              on={active}
              disabled={busy}
              onChange={toggleAccess}
              label="Driftnet access"
            />
          </section>
          {error && (
            <div className="notice home-notice" role="alert">
              <WarningCircle size={16} />
              <span>{error}</span>
              <button aria-label="Dismiss" onClick={() => setError("")}>
                <X size={14} />
              </button>
            </div>
          )}
        </>
      ) : (
        <>
          <header className="settings-header">
            <button
              className="icon-button"
              aria-label="Back"
              onClick={() => {
                setView("home");
                setError("");
              }}
            >
              <ArrowLeft size={22} />
            </button>
            <h2>Settings</h2>
            <span className="mini-wordmark" aria-label="Driftnet">
              d_
            </span>
          </header>

          <div className="tabs" aria-label="Item type">
            <button
              aria-pressed={tab === "site"}
              onClick={() => setTab("site")}
            >
              Websites
            </button>
            <button aria-pressed={tab === "app"} onClick={() => setTab("app")}>
              Apps
            </button>
          </div>
          {tab === "site" ? (
            <form
              className="add-form"
              onSubmit={(e) => {
                e.preventDefault();
                addSite();
              }}
            >
              <input
                aria-label="Website address"
                placeholder="example.com"
                value={site}
                onChange={(e) => setSite(e.target.value)}
                autoComplete="off"
              />
              <button
                className="add-button"
                aria-label="Add website"
                disabled={busy || !site.trim()}
              >
                <Plus size={20} />
              </button>
            </form>
          ) : (
            <button className="pick-app" disabled={busy} onClick={pickApp}>
              <Plus size={18} />
              Choose app
            </button>
          )}
          <div className="rules-list">
            {visibleRules.length ? (
              visibleRules.map((rule) => (
                <div className="rule" key={rule.id}>
                  <span
                    className={
                      "rule-icon" +
                      (rule.target.kind === "discord"
                        ? " discord-fallback"
                        : "")
                    }
                  >
                    <RuleIcon rule={rule} icon={icons[rule.id]} />
                  </span>
                  <div className="rule-label">
                    <strong>{ruleName(rule)}</strong>
                  </div>
                  <Toggle
                    label={ruleName(rule) + " enabled"}
                    on={rule.enabled}
                    disabled={busy}
                    onChange={() => changeRule(rule)}
                  />
                  <button
                    className="delete"
                    aria-label={ruleName(rule) + " — remove"}
                    disabled={busy}
                    onClick={() => remove(rule)}
                  >
                    <X size={16} />
                  </button>
                </div>
              ))
            ) : (
              <p className="empty">No items yet.</p>
            )}
          </div>
          {error && (
            <p className="notice" role="alert">
              <WarningCircle size={18} />
              {error}
            </p>
          )}
        </>
      )}
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
