import { RemoteSourceConnectDialog } from "@/components/menu/dialogs/remote-source/connect";
import { useConnectJellyfin, useJellyfinLogin } from "@/mutations/use-source-mutations";

export const JellyfinConnectDialog = () => (
  <RemoteSourceConnectDialog
    mode="jellyfin-connect"
    title="Connect to Jellyfin"
    description={
      <>
        Point Nightingale at a Jellyfin server. Audio is downloaded to your local cache on first
        analysis so the rest of the karaoke pipeline keeps working exactly like a folder library.
      </>
    }
    urlInputId="jelly-url"
    urlPlaceholder="https://jellyfin.example.com"
    usernameInputId="jelly-user"
    passwordInputId="jelly-pass"
    useLogin={useJellyfinLogin}
    useConnect={useConnectJellyfin}
  />
);
