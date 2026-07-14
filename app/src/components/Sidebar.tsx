import { Plug, Podcast, FolderSync, TriangleAlert, Lock, Speaker, Check, ChevronsUpDown } from "lucide-react";
import { motion } from "motion/react";
import { cn } from "@/lib/utils";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Logo } from "@/components/Logo";
import { useUi, type Section } from "@/store/ui";
import { useConnection } from "@/store/connection";
import { useDevices } from "@/store/devices";
import { useErrors } from "@/store/errors";

interface Item {
  id: Section;
  label: string;
  icon: typeof Plug;
  lockable: boolean;
}

const ITEMS: Item[] = [
  { id: "connect", label: "Connexion", icon: Plug, lockable: false },
  { id: "podcasts", label: "Podcasts", icon: Podcast, lockable: true },
  { id: "sync", label: "Synchroniser", icon: FolderSync, lockable: true },
  { id: "errors", label: "Erreurs", icon: TriangleAlert, lockable: false },
];

export function Sidebar() {
  const { section, setSection } = useUi();
  const isConnected = useConnection((s) => s.isConnected);
  const devices = useDevices((s) => s.devices);
  const setActiveDevice = useDevices((s) => s.setActive);
  const hasDevice = devices.length > 0;
  const activeDevice = devices.find((d) => d.isActive);
  const lastPingMs = useConnection((s) => s.lastPingMs);
  const errorCount = useErrors((s) => s.entries.length);

  return (
    <aside className="flex w-52 flex-col border-r bg-card/40">
      {}
      <div data-tauri-drag-region className="h-11 shrink-0" />
      <div className="flex items-center gap-2 px-3 pb-1">
        <Logo className="size-6 shrink-0 text-brand" />
        <span className="font-brand-serif text-xl text-brand select-none">
          Morgane
        </span>
      </div>

      <nav className="flex flex-1 flex-col gap-0.5 px-2 py-2">
        {ITEMS.map((item) => {

          const locked = item.lockable && !isConnected && !hasDevice;
          const active = section === item.id;
          const Icon = item.icon;
          return (
            <button
              key={item.id}
              disabled={locked}
              onClick={() => setSection(item.id)}
              className={cn(
                "group relative flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm transition-colors",
                active
                  ? "bg-primary/12 text-primary font-medium"
                  : "text-foreground/80 hover:bg-accent",
                locked && "cursor-not-allowed opacity-45 hover:bg-transparent",
              )}
            >
              {active && (
                <motion.span
                  layoutId="sidebar-active"
                  className="absolute inset-0 -z-10 rounded-md bg-primary/12"
                  transition={{ type: "spring", stiffness: 400, damping: 32 }}
                />
              )}
              <Icon className="size-4 shrink-0" />
              <span className="flex-1 text-left">{item.label}</span>
              {locked && <Lock className="size-3.5 opacity-70" />}
              {item.id === "errors" && errorCount > 0 && (
                <span className="rounded-full bg-destructive px-1.5 text-xs font-semibold text-white tabular-nums">
                  {errorCount}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="border-t px-3 py-2.5 select-none">
        <div className="flex items-center gap-2 text-sm">
          <span
            className={cn(
              "size-2 rounded-full",
              isConnected ? "bg-[var(--success)]" : "bg-muted-foreground/50",
            )}
          />
          <span className="text-foreground/80">
            {isConnected ? "Enceinte connectée" : "Non connectée"}
          </span>
        </div>
        {lastPingMs !== null && isConnected && (
          <div className="mt-0.5 pl-4 text-xs text-muted-foreground tabular-nums">
            ping {Math.round(lastPingMs)} ms
          </div>
        )}

        {}
        {hasDevice && (
          <DropdownMenu>
            <DropdownMenuTrigger className="mt-2 flex w-full items-center gap-2 rounded-md px-1.5 py-1.5 text-left outline-none hover:bg-accent">
              <Speaker className="size-4 shrink-0 text-muted-foreground" />
              <div
                className="min-w-0 flex-1 truncate text-xs font-medium"
                title={activeDevice ? `Merlin ${activeDevice.mac}` : undefined}
              >
                {activeDevice?.name ?? "Choisir une Merlin"}
              </div>
              <ChevronsUpDown className="size-3.5 shrink-0 text-muted-foreground/60" />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" side="top" className="w-56">
              {devices.map((device) => (
                <DropdownMenuItem
                  key={device.mac}
                  onSelect={() => void setActiveDevice(device.mac)}
                  className="gap-2"
                >
                  <Check
                    className={cn("size-4 shrink-0", device.isActive ? "opacity-100" : "opacity-0")}
                  />
                  <span className="min-w-0 flex-1 truncate text-sm" title={`Merlin ${device.mac}`}>
                    {device.name}
                  </span>
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </aside>
  );
}
