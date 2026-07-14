import { useState } from "react";
import { pickImage } from "@/lib/pickImage";

export interface ImageOverride {

  draft: string | null;
  editing: boolean;
  begin: (current: string) => void;
  cancel: () => void;
  setDraft: (value: string) => void;

  commitDraft: () => void | Promise<void>;

  browse: () => Promise<void>;
}

export function useImageOverride(
  onCommit: (source: string | null) => void | Promise<void>,
): ImageOverride {
  const [draft, setDraftState] = useState<string | null>(null);
  const commit = async (value: string | null) => {
    try {
      await onCommit(value?.trim() || null);
    } catch {

    } finally {
      setDraftState(null);
    }
  };
  return {
    draft,
    editing: draft !== null,
    begin: (current) => setDraftState(current),
    cancel: () => setDraftState(null),
    setDraft: (value) => setDraftState(value),
    commitDraft: () => commit(draft),
    browse: async () => {
      const path = await pickImage();
      if (path) await commit(path);
    },
  };
}
