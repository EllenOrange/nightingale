import { atomWithStorage } from "jotai/utils";
import { useAtom } from "jotai";

const donationSeenAtom = atomWithStorage<boolean>("nightingale:donation_seen", false);

export const useDonationSeen = () => {
  const [seen, setSeen] = useAtom(donationSeenAtom);

  return {
    seen,
    markSeen: () => setSeen(true),
  };
};
