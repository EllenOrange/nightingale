import { BrowserRouter, Routes, Route } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "./App.css";
import { Toaster } from "./components/ui/sonner";
import { TauriAppShell } from "./components/window/title-bar";
import { NavInputProvider } from "./contexts/nav-input-context";
import { MenuFocusProvider } from "./contexts/menu-focus-context";
import { MenuIndex, MenuLayout } from "./pages/menu/menu";
import { SettingsPage } from "./pages/menu/settings";
import { Playback } from "./pages/playback/playback";
import { ThemeProvider } from "./contexts/theme-context";
import { useConfig } from "./queries/use-config";
import { useUpdate } from "./queries/use-update";
import { TooltipProvider } from "./components/ui/tooltip";
import { UPDATES_SUPPORTED } from "./bridge/platform";
import { RemotePlayback } from "./hooks/party/use-remote-playback";
import { Party } from "./pages/party/party";
import { Admin } from "./pages/admin/admin";

const queryClient = new QueryClient();

const UpdateAutoCheck = () => {
  useUpdate();

  return null;
};

const InnerWrapper = () => (
  <>
    <MenuFocusProvider>
      <BrowserRouter>
        {/* Party layer: obeys remote play signals on any route (inert on Tauri). */}
        <RemotePlayback />
        <Routes>
          <Route path="/" element={<MenuLayout />}>
            <Route index element={<MenuIndex />} />
            <Route path="settings" element={<SettingsPage />} />
          </Route>
          <Route path="/playback" element={<Playback />} />
          <Route path="/party" element={<Party />} />
          <Route path="/admin" element={<Admin />} />
        </Routes>
      </BrowserRouter>
    </MenuFocusProvider>
    <Toaster />
    {UPDATES_SUPPORTED && <UpdateAutoCheck />}
  </>
);

const ThemeWrapper = () => {
  const { data: config } = useConfig();

  return (
    <ThemeProvider defaultTheme={config?.dark_mode === false ? "light" : "dark"}>
      <TooltipProvider>
        <TauriAppShell>
          <InnerWrapper />
        </TauriAppShell>
      </TooltipProvider>
    </ThemeProvider>
  );
};

const App = () => (
  <NavInputProvider>
    <QueryClientProvider client={queryClient}>
      <ThemeWrapper />
    </QueryClientProvider>
  </NavInputProvider>
);

export default App;
