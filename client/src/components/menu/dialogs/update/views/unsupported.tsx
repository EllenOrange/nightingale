import { ExternalLinkIcon, InfoIcon } from "lucide-react";
import { CURRENT_VERSION, InfoLine, PairFooter, type FocusCtx, type ViewParts } from "../parts";

interface Args {
  ctx: FocusCtx;
  onClose: () => void;
  onOpenReleases: () => void;
}

export const unsupportedView = ({ ctx, onClose, onOpenReleases }: Args): ViewParts => ({
  description: "Linux builds use manual updates from GitHub Releases.",
  body: (
    <InfoLine icon={InfoIcon}>
      Auto-update isn't supported on Linux. Download the latest release from GitHub to update
      Nightingale (you're on v{CURRENT_VERSION}).
    </InfoLine>
  ),
  footer: (
    <PairFooter
      ctx={ctx}
      onClose={onClose}
      primaryLabel="Open GitHub Releases"
      primaryIcon={ExternalLinkIcon}
      onPrimary={onOpenReleases}
    />
  ),
});
