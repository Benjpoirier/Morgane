import { useState } from "react";
import { Loader2, CheckCircle2, Circle, Trash2 } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Logo } from "@/components/Logo";
import { testConnection } from "@/lib/ipc";
import { useConnection } from "@/store/connection";
import { useDevices } from "@/store/devices";
import { useInlineEdit } from "@/hooks/useInlineEdit";
import type { RegisteredDevice } from "@/lib/types";
import { cn } from "@/lib/utils";

const RING_DELAYS = [0, 1.3, 2.6];

const ORB_VIOLET =
  "radial-gradient(circle at 35% 30%, oklch(0.82 0.13 320), oklch(0.58 0.22 300) 55%, oklch(0.4 0.2 295))";
const ORB_GREEN =
  "radial-gradient(circle at 35% 30%, oklch(0.85 0.16 155), oklch(0.6 0.17 150) 55%, oklch(0.4 0.12 150))";
const GLOW_VIOLET =
  "0 0 60px oklch(0.6 0.22 305 / 0.7), inset 0 -14px 30px oklch(0.3 0.15 290 / 0.8)";
const GLOW_GREEN =
  "0 0 60px oklch(0.72 0.19 149 / 0.55), inset 0 -14px 30px oklch(0.3 0.12 150 / 0.7)";

export function ConnectView() {
  const {
    host,
    port,
    isConnected,
    isTesting,
    statusMessage,
    lastPingMs,
    setHost,
    setPort,
    setTesting,
    applyStatus,
  } = useConnection();
  const devices = useDevices((s) => s.devices);
  const [portText, setPortText] = useState(String(port));

  const handleTest = async () => {
    setTesting(true);
    try {
      const status = await testConnection(host, port, true);
      applyStatus(status);
    } catch (e) {
      applyStatus({
        connected: false,
        latencyMs: null,
        message: `Erreur : ${e}`,
        busy: false,
        deviceMac: null,
        deviceName: null,
        newlyRegistered: false,
      });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="flex h-full items-center justify-center overflow-y-auto p-8">
      <div className="flex w-full max-w-sm flex-col items-center text-center">
        <button
          onClick={handleTest}
          disabled={isTesting}
          title={isConnected ? "Retester la connexion" : "Se connecter"}
          className="relative mb-7 grid size-[220px] shrink-0 place-items-center rounded-full outline-none disabled:cursor-wait"
          style={{ animation: "n-float 7s ease-in-out infinite" }}
        >
          {!isConnected &&
            RING_DELAYS.map((d) => (
              <span
                key={d}
                className="absolute inset-0 rounded-full"
                style={{
                  border: "1.5px solid oklch(0.7 0.16 305 / 0.5)",
                  animation: `n-ring 4s ease-out ${d}s infinite`,
                }}
              />
            ))}
          <div
            className="absolute inset-[34px] rounded-full transition-all duration-500"
            style={{
              background: isConnected ? ORB_GREEN : ORB_VIOLET,
              boxShadow: isConnected ? GLOW_GREEN : GLOW_VIOLET,
              opacity: isTesting ? 0.85 : 1,
            }}
          />
          {isTesting ? (
            <Loader2 className="relative size-12 animate-spin text-white" strokeWidth={1.6} />
          ) : (
            <Logo className="relative size-14 text-white" />
          )}
          <span
            className="absolute top-3 right-10 size-4 rounded-full"
            style={{
              background: "oklch(0.92 0.09 85)",
              boxShadow: "0 0 16px oklch(0.9 0.12 85)",
            }}
          />
        </button>

        <h1 className="font-brand-serif text-3xl">
          {isConnected
            ? "Enceinte connectée"
            : isTesting
              ? "Recherche de l'enceinte…"
              : "Enceinte non connectée"}
        </h1>
        {isConnected ? (
          <p className="mt-1 text-sm text-muted-foreground tabular-nums">
            {host}:{port}
            {lastPingMs !== null && ` · ${Math.round(lastPingMs)} ms`}
          </p>
        ) : (
          !isTesting && (
            <p className="mt-1 text-sm text-muted-foreground">
              Clique l'orbe pour rejoindre ta Merlin.
            </p>
          )
        )}

        <div className="mt-7 grid w-full grid-cols-[auto_1fr] items-center gap-x-3 gap-y-2 text-left">
          <label className="text-sm text-muted-foreground">Hôte</label>
          <Input value={host} onChange={(e) => setHost(e.target.value)} />
          <label className="text-sm text-muted-foreground">Port</label>
          <Input
            value={portText}
            onChange={(e) => {
              setPortText(e.target.value);
              const parsed = Number.parseInt(e.target.value, 10);
              if (Number.isFinite(parsed) && parsed > 0 && parsed <= 65535) {
                setPort(parsed);
              }
            }}
          />
        </div>

        {statusMessage && (
          <p className="mt-3 font-mono text-xs break-all text-muted-foreground select-text">
            {statusMessage}
          </p>
        )}

        {devices.length > 0 && (
          <div className="mt-8 w-full text-left">
            <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Mes Merlin
            </div>
            <div className="flex flex-col gap-1.5">
              {devices.map((device) => (
                <DeviceRow key={device.mac} device={device} />
              ))}
            </div>
            <p className="mt-2 text-xs text-muted-foreground">
              L'enceinte active définit l'état « déjà synchronisé » affiché. La recherche de
              podcasts se fait hors connexion, sur ton WiFi habituel.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

function DeviceRow({ device }: { device: RegisteredDevice }) {
  const setActive = useDevices((s) => s.setActive);
  const remove = useDevices((s) => s.remove);
  const rename = useDevices((s) => s.rename);
  const name = useInlineEdit(async (value) => {
    const next = value.trim();
    if (next && next !== device.name) await rename(device.mac, next);
  });

  return (
    <div
      className={cn(
        "flex items-center gap-2 rounded-md border px-3 py-2",
        device.isActive ? "border-primary bg-primary/10" : "hover:bg-accent",
      )}
    >
      <button
        onClick={() => setActive(device.mac)}
        title={device.isActive ? "Enceinte active" : "Rendre active"}
        className="shrink-0"
      >
        {device.isActive ? (
          <CheckCircle2 className="size-4 text-primary" />
        ) : (
          <Circle className="size-4 text-muted-foreground" />
        )}
      </button>
      <div className="min-w-0 flex-1">
        {name.editing ? (
          <Input defaultValue={device.name} {...name.inputProps} className="h-7" />
        ) : (
          <div
            className="truncate text-sm font-medium"
            onDoubleClick={name.begin}
            title={`Double-clic pour renommer · ${device.mac}`}
          >
            {device.name}
          </div>
        )}
      </div>
      <button
        onClick={() => remove(device.mac)}
        title="Retirer cette Merlin"
        className="shrink-0 rounded p-1 text-muted-foreground hover:text-destructive"
      >
        <Trash2 className="size-4" />
      </button>
    </div>
  );
}
