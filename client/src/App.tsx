import { BrowserRouter, Routes, Route } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "./App.css";
import { Toaster } from "./components/ui/sonner";
import { TauriAppShell } from "./components/window/title-bar";
import { NavInputProvider } from "./contexts/nav-input-context";
import { Menu } from "./pages/menu/menu";
import { Playback } from "./pages/playback/playback";
import { ThemeProvider } from "./contexts/theme-context";
import { useConfig } from "./queries/use-config";
import { useUpdate } from "./queries/use-update";
import { Setup } from "./components/menu/dialogs/setup";
import { TooltipProvider } from "./components/ui/tooltip";

const queryClient = new QueryClient();

const UpdateAutoCheck = () => {
  useUpdate();

  return null;
};

const InnerWrapper = () => (
  <>
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Menu />} />
        <Route path="/playback" element={<Playback />} />
      </Routes>
    </BrowserRouter>
    <Toaster />
    <Setup />
    <UpdateAutoCheck />
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
