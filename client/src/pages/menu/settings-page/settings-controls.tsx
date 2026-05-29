import { Button } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { FieldDescription } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { ReactNode } from "react";
import { NUMBER_PICKER_SIZE, type SettingsOption } from "./constants";

interface SettingsSelectProps {
  id?: string;
  label: string;
  placeholder: string;
  value: string;
  options: SettingsOption[];
  triggerClassName?: string;
  onValueChange: (value: string) => void;
}

export function SettingsSelect({
  id,
  label,
  placeholder,
  value,
  options,
  triggerClassName,
  onValueChange,
}: SettingsSelectProps) {
  return (
    <Select onValueChange={onValueChange} value={value}>
      <SelectTrigger id={id} className={triggerClassName}>
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          <SelectLabel>{label}</SelectLabel>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  );
}

interface NumberButtonGroupProps {
  name: string;
  value: number;
  segment: number;
  getFocusClassName: (segment: number, slot?: number) => string;
  onChange: (value: number) => void;
}

export function NumberButtonGroup({
  name,
  value,
  segment,
  getFocusClassName,
  onChange,
}: NumberButtonGroupProps) {
  return (
    <ButtonGroup className="scrollbar-hide w-full max-w-full justify-start overflow-x-auto">
      {Array.from({ length: NUMBER_PICKER_SIZE }, (_, index) => {
        const option = index + 1;

        return (
          <Button
            key={`${name}-${option}`}
            onClick={() => onChange(option)}
            variant={value === option ? "default" : "outline"}
            className={getFocusClassName(segment, index)}
          >
            {option}
          </Button>
        );
      })}
    </ButtonGroup>
  );
}

export function PageHeader() {
  return (
    <div className="space-y-1">
      <h1 className="text-xl font-semibold tracking-tight sm:text-2xl">Settings</h1>
      <p className="text-sm text-muted-foreground">
        Tune display, microphone, stem separation, transcript engine, and model parameters.
      </p>
    </div>
  );
}

export function Hint({ children }: { children: ReactNode }) {
  return <FieldDescription>{children}</FieldDescription>;
}
