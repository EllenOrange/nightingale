import { RemoteSourceConnectDialog } from "@/components/menu/dialogs/remote-source/connect";
import { useConnectNavidrome, useNavidromeLogin } from "@/mutations/use-source-mutations";

export const NavidromeConnectDialog = () => (
  <RemoteSourceConnectDialog
    mode="navidrome-connect"
    title="Connect to Navidrome"
    description={
      <>
        Point Nightingale at a Navidrome (Subsonic-compatible) server. Audio is downloaded to your
        local cache on first analysis so the rest of the karaoke pipeline keeps working exactly like
        a folder library.
      </>
    }
    urlInputId="navi-url"
    urlPlaceholder="https://navidrome.example.com"
    usernameInputId="navi-user"
    passwordInputId="navi-pass"
    useLogin={useNavidromeLogin}
    useConnect={useConnectNavidrome}
  />
);
