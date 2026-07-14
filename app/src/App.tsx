import { useEffect } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Sidebar } from "./components/Sidebar";
import { Logo } from "./components/Logo";
import { Starfield } from "./components/Starfield";
import { NameDeviceDialog } from "./components/NameDeviceDialog";
import { ConnectView } from "./views/ConnectView";
import { ErrorsView } from "./views/ErrorsView";
import { PodcastsView } from "./views/PodcastsView";
import { SyncView } from "./views/SyncView";
import { SetupView } from "./views/SetupView";
import { useConnectionPolling } from "./lib/usePolling";
import { useSyncListeners } from "./lib/useSyncListeners";
import { useAppBootstrap } from "./hooks/useAppBootstrap";
import { useConnectionEffects } from "./hooks/useConnectionEffects";
import { usePrepareOrchestration } from "./hooks/usePrepareOrchestration";
import { useUi } from "./store/ui";
import { useSetup } from "./store/setup";

const NIGHT_BG =
  "radial-gradient(140% 100% at 50% -10%, oklch(0.28 0.09 295) 0%, oklch(0.17 0.06 285) 42%, oklch(0.11 0.04 280) 100%)";

export function App() {
  const ready = useSetup((s) => s.ready);
  const checkSetup = useSetup((s) => s.check);

  useEffect(() => {
    checkSetup();
  }, [checkSetup]);

  return (
    <div className="relative h-full overflow-hidden" style={{ background: NIGHT_BG }}>
      <Starfield count={80} />
      <div className="relative z-10 h-full">
        {ready === null ? (
          <div className="flex h-full w-full items-center justify-center">
            <Logo className="size-14 text-brand/70" />
          </div>
        ) : ready === false ? (
          <SetupView />
        ) : (
          <MainApp />
        )}
      </div>
    </div>
  );
}

function MainApp() {
  useConnectionPolling();
  useSyncListeners();
  useAppBootstrap();
  useConnectionEffects();
  usePrepareOrchestration();
  const section = useUi((s) => s.section);

  return (
    <div className="flex h-full">
      <NameDeviceDialog />
      <Sidebar />
      <main className="min-w-0 flex-1 overflow-hidden">
        <AnimatePresence mode="wait">
          <motion.div
            key={section}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className="h-full"
          >
            {section === "connect" && <ConnectView />}
            {section === "errors" && <ErrorsView />}
            {section === "podcasts" && <PodcastsView />}
            {section === "sync" && <SyncView />}
          </motion.div>
        </AnimatePresence>
      </main>
    </div>
  );
}
