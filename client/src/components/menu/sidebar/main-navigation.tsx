import {
  SidebarContent,
  SidebarGroup,
  SidebarMenu,
  SidebarMenuSkeleton,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubItem,
} from "@/components/ui/sidebar";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import {
  ChevronDown,
  Flame,
  FileQuestionMark,
  DiscIcon,
  UserIcon,
  type LucideIcon,
} from "lucide-react";
import { useLibraryMenuItems } from "@/queries/use-library-menu-items";
import type { LibraryMenuItem } from "@/types/LibraryMenuItem";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import {
  isLibraryMenuItemActive,
  libraryFilterFromMenuSelection,
  type LibraryMenuSection,
} from "@/lib/library-menu-filter";
import type { LibraryMenuFilters } from "@/types/LibraryMenuFilters";
import { useLibraryFilter } from "@/hooks/use-library-filter";
import { useMenuFocus } from "@/contexts/menu-focus-context";
import {
  SidebarNavProvider,
  useSidebarRowFocus,
  type SidebarNavRow,
} from "@/contexts/sidebar-nav-context";
import { FolderActions } from "./folder-actions";

type NavSectionConfig = {
  section: LibraryMenuSection;
  label: string;
  icon: LucideIcon;
  defaultOpen: boolean;
};

const NAV_SECTIONS: NavSectionConfig[] = [
  { section: "hot", label: "Quick Filters", icon: Flame, defaultOpen: true },
  { section: "no_metadata", label: "No Metadata", icon: FileQuestionMark, defaultOpen: false },
  { section: "artists", label: "Artists", icon: UserIcon, defaultOpen: false },
  { section: "albums", label: "Albums", icon: DiscIcon, defaultOpen: false },
];

interface MenuItemCountsProps {
  item: LibraryMenuItem;
}

function MenuItemCounts({ item }: MenuItemCountsProps) {
  return (
    <div className="flex gap-1">
      {item.count !== item.analysedCount && (
        <Badge className="h-5 border-0 bg-muted px-1.5 text-[0.65rem] font-medium text-muted-foreground">
          {item.count.toString()}
        </Badge>
      )}
      {item.analysedCount > 0n && (
        <Badge className="h-5 border-0 bg-chart-3/15 px-1.5 text-[0.65rem] font-medium text-chart-3 dark:bg-chart-3/20">
          {item.analysedCount.toString()}
        </Badge>
      )}
    </div>
  );
}

interface LibraryNavSubItemProps {
  section: LibraryMenuSection;
  item: LibraryMenuItem;
  filter: LibraryMenuFilters;
  onSelectItem: (section: LibraryMenuSection, item: LibraryMenuItem) => void;
}

function LibraryNavSubItem({ section, item, filter, onSelectItem }: LibraryNavSubItemProps) {
  const { isSidebarActive, isItemFocused, itemIndex } = useSidebarRowFocus(section, item.value);
  return (
    <SidebarMenuSubItem>
      <SidebarMenuButton
        data-sidebar-nav-index={itemIndex}
        isActive={isLibraryMenuItemActive(section, item, filter)}
        className={`flex h-fit items-center justify-between gap-2 px-2 py-1.5 ${
          isSidebarActive && isItemFocused ? "ring-2 ring-primary bg-sidebar-accent" : ""
        }`}
        onClick={() => onSelectItem(section, item)}
      >
        {item.label}
        <MenuItemCounts item={item} />
      </SidebarMenuButton>
    </SidebarMenuSubItem>
  );
}

interface LibraryNavSectionProps extends Omit<NavSectionConfig, "defaultOpen"> {
  items: LibraryMenuItem[];
  filter: LibraryMenuFilters;
  open: boolean;
  onToggleOpen: (open: boolean) => void;
  onSelectItem: (section: LibraryMenuSection, item: LibraryMenuItem) => void;
}

function LibraryNavSection({
  section,
  label,
  icon: Icon,
  items,
  filter,
  open,
  onToggleOpen,
  onSelectItem,
}: LibraryNavSectionProps) {
  const { isSidebarActive, isCollapseFocused, collapseIndex } = useSidebarRowFocus(section);

  return (
    <Collapsible open={open} onOpenChange={onToggleOpen} className="group/collapsible">
      <SidebarMenuItem>
        <CollapsibleTrigger asChild>
          <SidebarMenuButton
            data-sidebar-nav-index={collapseIndex}
            className={`flex w-full justify-between ${
              isSidebarActive && isCollapseFocused ? "ring-2 ring-primary bg-sidebar-accent" : ""
            }`}
          >
            <span className="flex items-center gap-2">
              <Icon className="size-4 shrink-0" />
              {label}
            </span>
            <ChevronDown className="transition-transform group-data-[state=open]/collapsible:rotate-180" />
          </SidebarMenuButton>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <SidebarMenuSub className="mr-0 pr-0">
            {items.map((item) => (
              <LibraryNavSubItem
                key={`${section}:${item.value}`}
                section={section}
                item={item}
                filter={filter}
                onSelectItem={onSelectItem}
              />
            ))}
          </SidebarMenuSub>
        </CollapsibleContent>
      </SidebarMenuItem>
    </Collapsible>
  );
}

interface MainNavigationProps {
  baseIndex: number;
  registerCallbacks: (callbacks: (() => void)[]) => void;
  folderFocusedSidebarIndex: number;
  registerFolderCallback: (callback: ((subIndex: number) => void) | null) => void;
}

export const MainNavigation = ({
  baseIndex,
  registerCallbacks,
  folderFocusedSidebarIndex,
  registerFolderCallback,
}: MainNavigationProps) => {
  const { data: menu } = useLibraryMenuItems();
  const { setLibraryFilter, ...filter } = useLibraryFilter();
  const { focus } = useMenuFocus();
  const [openBySection, setOpenBySection] = useState<Record<LibraryMenuSection, boolean>>(
    () =>
      Object.fromEntries(
        NAV_SECTIONS.map(({ section, defaultOpen }) => [section, defaultOpen]),
      ) as Record<LibraryMenuSection, boolean>,
  );

  const selectMenuItem = useCallback(
    (section: LibraryMenuSection, item: LibraryMenuItem) => {
      setLibraryFilter(libraryFilterFromMenuSelection(section, item));
    },
    [setLibraryFilter],
  );

  const visibleSections = useMemo(() => {
    if (!menu) {
      return [];
    }

    return NAV_SECTIONS.map((config) => ({
      ...config,
      visibleItems: menu[config.section].filter(({ count }) => count > 0n),
    })).filter(({ visibleItems }) => visibleItems.length > 0);
  }, [menu]);

  const rows = useMemo<SidebarNavRow[]>(() => {
    return visibleSections.flatMap(({ section, visibleItems }) => {
      const sectionRows: SidebarNavRow[] = [{ kind: "collapse", section }];
      if (openBySection[section]) {
        sectionRows.push(
          ...visibleItems.map(({ value }) => ({ kind: "item" as const, section, value })),
        );
      }
      return sectionRows;
    });
  }, [visibleSections, openBySection]);

  useEffect(() => {
    const callbacks = rows.map((row) => {
      if (row.kind === "collapse") {
        return () => {
          setOpenBySection((prev) => ({ ...prev, [row.section]: !prev[row.section] }));
        };
      }

      return () => {
        const item = menu?.[row.section].find((entry) => entry.value === row.value);
        if (!item) {
          return;
        }
        selectMenuItem(row.section, item);
      };
    });

    registerCallbacks(callbacks);

    return () => {
      registerCallbacks([]);
    };
  }, [rows, menu, selectMenuItem, registerCallbacks]);

  const isSidebarActive = focus.active && focus.panel === "sidebar";

  useEffect(() => {
    if (!isSidebarActive) {
      return;
    }

    const rafId = requestAnimationFrame(() => {
      const focusedItem = document.querySelector<HTMLElement>(
        `[data-sidebar-nav-index="${focus.sidebarIndex}"]`,
      );
      focusedItem?.scrollIntoView({ block: "nearest" });
    });

    return () => cancelAnimationFrame(rafId);
  }, [focus.sidebarIndex, isSidebarActive, rows]);

  const showEmptyPlaceholder = !menu || visibleSections.length === 0;

  return (
    <SidebarContent>
      <FolderActions
        focusedSidebarIndex={folderFocusedSidebarIndex}
        registerCallback={registerFolderCallback}
      />
      <SidebarGroup>
        <SidebarMenu>
          {showEmptyPlaceholder ? (
            <SidebarMenuItem className="px-1 py-2">
              <div className="space-y-1">
                <SidebarMenuSkeleton showIcon />
                <SidebarMenuSkeleton showIcon />
                <SidebarMenuSkeleton showIcon />
              </div>
            </SidebarMenuItem>
          ) : (
            <SidebarNavProvider rows={rows} baseIndex={baseIndex}>
              {visibleSections.map((config) => (
                <LibraryNavSection
                  key={config.section}
                  section={config.section}
                  label={config.label}
                  icon={config.icon}
                  items={config.visibleItems}
                  filter={filter}
                  open={openBySection[config.section]}
                  onToggleOpen={(open) => {
                    setOpenBySection((prev) => ({ ...prev, [config.section]: open }));
                  }}
                  onSelectItem={selectMenuItem}
                />
              ))}
            </SidebarNavProvider>
          )}
        </SidebarMenu>
      </SidebarGroup>
    </SidebarContent>
  );
};
