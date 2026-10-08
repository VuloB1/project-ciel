import * as SliderPrimitive from "@radix-ui/react-slider";

export interface SliderProps {
  value: number;
  onValueChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  label: string;
  disabled?: boolean;
}

/** A single 0-100 personality slider (sección 11 del brief). */
export function Slider({ value, onValueChange, min = 0, max = 100, step = 1, label, disabled }: SliderProps) {
  return (
    <SliderPrimitive.Root
      value={[value]}
      onValueChange={([next]) => {
        if (next !== undefined) onValueChange(next);
      }}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      aria-label={label}
      className="relative flex h-5 w-full touch-none select-none items-center data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50"
    >
      <SliderPrimitive.Track className="relative h-1.5 w-full grow rounded-full bg-surface-secondary">
        <SliderPrimitive.Range className="absolute h-full rounded-full bg-accent" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        className="block h-4 w-4 rounded-full bg-white shadow transition-transform hover:scale-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
      />
    </SliderPrimitive.Root>
  );
}
