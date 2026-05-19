import { SidebarHeader } from "@/components/ui/sidebar";

import logoUrl from "@/assets/images/logo.png";
import { ThemeToggle } from "./theme-toggle";

export const Header = () => (
  <SidebarHeader>
    <div className="relative flex w-full items-center justify-center pb-2">
      <img src={logoUrl} className="w-5/6" />
      <ThemeToggle className="absolute right-1 top-0" />
    </div>
  </SidebarHeader>
);
