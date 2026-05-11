import { Progress } from "@/components/ui/progress";
import prettyBytes from "pretty-bytes";
import { GENERIC_DESCRIPTION, type ViewParts } from "../parts";

interface Args {
  downloaded: number;
  contentLength: number | null;
  version: string;
}

export const downloadingView = ({ downloaded, contentLength, version }: Args): ViewParts => {
  const percent =
    contentLength && contentLength > 0
      ? Math.min(100, Math.floor((downloaded / contentLength) * 100))
      : null;

  const sizeLabel = contentLength
    ? `${prettyBytes(downloaded)} of ${prettyBytes(contentLength)}${
        percent !== null ? ` (${percent}%)` : ""
      }`
    : prettyBytes(downloaded);

  return {
    description: GENERIC_DESCRIPTION,
    body: (
      <div className="flex flex-col gap-3">
        <p className="text-xs text-muted-foreground">Downloading {version}…</p>
        <Progress value={percent ?? 0} max={100} />
        <p className="text-xs text-muted-foreground">{sizeLabel}</p>
      </div>
    ),
    footer: null,
  };
};
