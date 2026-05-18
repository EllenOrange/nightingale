import { isAppReady } from "@/bridge/setup";
import { atom, useAtom } from "jotai";
import { useEffect } from "react";

const shouldRunSetupAtom = atom(false);

export const useShouldRunSetup = () => {
  const [shouldRunSetup, setShouldRunSetup] = useAtom(shouldRunSetupAtom);

  useEffect(() => {
    // Check, if setup is required
    const checkIsAppReady = async () => {
      return await isAppReady();
    };

    checkIsAppReady().then((isAppReady) => setShouldRunSetup(!isAppReady));
  }, []);

  return { shouldRunSetup, setShouldRunSetup };
};
