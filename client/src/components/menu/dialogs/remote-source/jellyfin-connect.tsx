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
    selection={{
      label: "Libraries to import",
      emptyMessage:
        "This server didn't report any libraries, so everything the account can see will be imported.",
      getItems: (login) =>
        login.libraries.map((library) => ({ id: library.id, label: library.name })),
    }}
  />
);
