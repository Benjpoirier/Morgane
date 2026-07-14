import { useState, type KeyboardEvent, type FocusEvent } from "react";

export interface InlineEdit {
  editing: boolean;
  begin: () => void;
  cancel: () => void;

  inputProps: {
    autoFocus: true;
    onKeyDown: (e: KeyboardEvent<HTMLInputElement>) => void;
    onBlur: (e: FocusEvent<HTMLInputElement>) => void;
  };
}

export function useInlineEdit(
  onCommit: (value: string) => void | Promise<void>,
): InlineEdit {
  const [editing, setEditing] = useState(false);
  const commit = async (value: string) => {
    try {
      await onCommit(value);
    } catch {

    } finally {
      setEditing(false);
    }
  };
  return {
    editing,
    begin: () => setEditing(true),
    cancel: () => setEditing(false),
    inputProps: {
      autoFocus: true,
      onKeyDown: (e) => {
        if (e.key === "Enter") void commit(e.currentTarget.value);
        else if (e.key === "Escape") setEditing(false);
      },
      onBlur: (e) => void commit(e.currentTarget.value),
    },
  };
}
