import { Song } from "@/types/Song";
import { atom, useAtom } from "jotai";

export type DialogMode =
  | "exit"
  | "create-profile"
  | "select-profile"
  | "settings"
  | "about"
  | "update"
  | { mode: "language"; song: Song }
  | null;

const dialogAtom = atom<DialogMode>(null);

export const useDialog = () => {
  const [mode, setMode] = useAtom(dialogAtom);

  return {
    mode,
    setMode,
    close() {
      setMode(null);
    },
  };
};
