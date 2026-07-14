import { useEffect, useRef } from "react";
import { useConnection } from "@/store/connection";
import { useDevices } from "@/store/devices";
import { useErrors } from "@/store/errors";
import { useTree } from "@/store/tree";
import { useUi } from "@/store/ui";

export function useConnectionEffects() {
  const isConnected = useConnection((s) => s.isConnected);
  const deviceCount = useDevices((s) => s.devices.length);
  const section = useUi((s) => s.section);
  const setSection = useUi((s) => s.setSection);
  const wasConnected = useRef(isConnected);

  useEffect(() => {
    if (isConnected) {
      const { host, port } = useConnection.getState();
      void useTree.getState().refresh(host, port);
    }
  }, [isConnected]);

  useEffect(() => {
    const locked = deviceCount === 0 && (section === "sync" || section === "podcasts");
    if (wasConnected.current && !isConnected && locked) {
      setSection("connect");
      useErrors
        .getState()
        .record("Connexion", "Connexion à l'enceinte perdue - retour à l'onglet Connexion.");
    }
    wasConnected.current = isConnected;
  }, [isConnected, section, setSection, deviceCount]);
}
