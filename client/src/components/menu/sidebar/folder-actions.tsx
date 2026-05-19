import { Button } from "@/components/ui/button";
import { SidebarGroup, SidebarMenu, SidebarMenuItem } from "@/components/ui/sidebar";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useMenuFocus } from "@/contexts/menu-focus-context";
import { useFolderActions } from "@/hooks/use-folder-actions";
import { cn } from "@/lib/utils";
import { FolderIcon, LibraryBigIcon, RefreshCwIcon, type LucideIcon } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";

interface FolderButton {
  icon: LucideIcon;
  label: string;
  handler: () => Promise<void> | void;
  disabled: boolean;
}

interface FolderActionsProps {
  focusedSidebarIndex: number;
  registerCallback: (callback: ((subIndex: number) => void) | null) => void;
}

export const FolderActions = ({ focusedSidebarIndex, registerCallback }: FolderActionsProps) => {
  const { focus, actionsRef } = useMenuFocus();
  const { rescanFolder, rescanFolderDisabled, selectFolder } = useFolderActions();

  const buttons = useMemo<FolderButton[]>(
    () => [
      {
        icon: FolderIcon,
        label: "Select folder",
        handler: selectFolder,
        disabled: false,
      },
      {
        icon: RefreshCwIcon,
        label: "Rescan folder",
        handler: rescanFolder,
        disabled: rescanFolderDisabled,
      },
    ],
    [rescanFolder, rescanFolderDisabled, selectFolder],
  );

  const buttonsRef = useRef(buttons);
  buttonsRef.current = buttons;

  useEffect(() => {
    const map = actionsRef.current.sidebarSubCountByIndex;
    map.set(focusedSidebarIndex, buttons.length);

    registerCallback((subIndex: number) => {
      const button = buttonsRef.current[subIndex];
      if (!button || button.disabled) {
        return;
      }
      button.handler();
    });

    return () => {
      map.delete(focusedSidebarIndex);
      registerCallback(null);
    };
  }, [actionsRef, focusedSidebarIndex, registerCallback, buttons.length]);

  const isSidebarActive = focus.active && focus.panel === "sidebar";
  const isClusterFocused = isSidebarActive && focus.sidebarIndex === focusedSidebarIndex;

  return (
    <SidebarGroup>
      <SidebarMenu>
        <SidebarMenuItem>
          <div
            data-sidebar-nav-index={focusedSidebarIndex}
            className="flex w-full items-center justify-between gap-2 rounded-[calc(var(--radius-sm)+2px)] px-2 py-1.5 text-xs"
          >
            <span className="flex items-center gap-2 text-sidebar-foreground/70">
              <LibraryBigIcon className="size-4 shrink-0" />
              Library
            </span>
            <div className="flex items-center gap-0.5">
              {buttons.map((button, index) => {
                const Icon = button.icon;
                const isButtonFocused = isClusterFocused && focus.sidebarSubIndex === index;

                return (
                  <Tooltip key={button.label}>
                    <TooltipTrigger asChild>
                      <Button
                        tabIndex={-1}
                        variant="ghost"
                        size="icon-xs"
                        aria-label={button.label}
                        disabled={button.disabled}
                        onClick={() => button.handler()}
                        data-sidebar-sub-index={index}
                        className={cn(
                          "text-sidebar-foreground/70 hover:bg-transparent hover:text-sidebar-foreground/70 focus-visible:ring-0 focus-visible:border-transparent dark:hover:bg-transparent",
                          isButtonFocused && "ring-2 ring-primary bg-sidebar-accent",
                        )}
                      >
                        <Icon />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>{button.label}</TooltipContent>
                  </Tooltip>
                );
              })}
            </div>
          </div>
        </SidebarMenuItem>
      </SidebarMenu>
      <div aria-hidden className="mx-auto mt-1 h-px w-full rounded-full bg-sidebar-border/80" />
    </SidebarGroup>
  );
};
