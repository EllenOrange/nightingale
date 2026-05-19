import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useTheme } from "@/contexts/theme-context";
import { cn } from "@/lib/utils";
import { useConfigMutation } from "@/mutations/use-config-mutation";
import { MoonIcon, SunIcon } from "lucide-react";
import { useMemo } from "react";

interface ThemeToggleProps {
  className?: string;
}

export const ThemeToggle = ({ className }: ThemeToggleProps) => {
  const { toggle, theme } = useTheme();
  const { mutate } = useConfigMutation();

  const { Icon, label } = useMemo(() => {
    return theme === "dark"
      ? { Icon: SunIcon, label: "Light mode" }
      : { Icon: MoonIcon, label: "Dark mode" };
  }, [theme]);

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          tabIndex={-1}
          variant="ghost"
          size="icon-xs"
          aria-label={label}
          onClick={() => {
            toggle();
            mutate({ dark_mode: theme !== "dark" });
          }}
          className={cn(
            "text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-0 focus-visible:border-transparent",
            className,
          )}
        >
          <Icon />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
};
