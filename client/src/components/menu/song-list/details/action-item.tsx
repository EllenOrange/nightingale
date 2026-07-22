import { Button } from "@/components/ui/button";
import type { LucideIcon } from "lucide-react";

export interface ActionItemProps {
  icon: LucideIcon;
  title: string;
  description: string;
  onClick: () => void | Promise<void>;
  disabled?: boolean;
  destructive?: boolean;
}

export const ActionItem = ({
  icon: Icon,
  title,
  description,
  onClick,
  disabled,
  destructive,
}: ActionItemProps) => (
  <Button
    type="button"
    variant={destructive ? "destructive" : "ghost"}
    size="lg"
    className="h-auto min-h-10 w-full items-start justify-start gap-2 px-2 py-1.5 text-left whitespace-normal"
    disabled={disabled}
    onClick={onClick}
  >
    <Icon className="mt-0.5 size-4" />
    <span className="min-w-0">
      <span className="block text-xs font-medium leading-tight">{title}</span>
      <span
        className={
          destructive
            ? "mt-0.5 block text-[0.625rem] leading-tight text-destructive/70"
            : "mt-0.5 block text-[0.625rem] leading-tight text-muted-foreground"
        }
      >
        {description}
      </span>
    </span>
  </Button>
);
