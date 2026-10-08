import { forwardRef, useImperativeHandle, useLayoutEffect, useRef, type TextareaHTMLAttributes } from "react";

import { cn } from "@/lib/theme/cn";

export interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  /** Grows with its content (never shorter than `rows`) and drops the manual
   * resize handle — same behaviour as the chat composer, without a cap. */
  autoGrow?: boolean;
}

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ className, autoGrow = false, style, ...props }, ref) => {
    const innerRef = useRef<HTMLTextAreaElement>(null);
    useImperativeHandle(ref, () => innerRef.current as HTMLTextAreaElement);

    useLayoutEffect(() => {
      const el = innerRef.current;
      if (!autoGrow || !el) return;
      const fit = () => {
        // Reset before measuring so scrollHeight reflects the current content,
        // not a taller height left over from before text was deleted.
        el.style.height = "auto";
        const borders = parseFloat(getComputedStyle(el).borderTopWidth) + parseFloat(getComputedStyle(el).borderBottomWidth);
        el.style.height = `${el.scrollHeight + borders}px`;
      };
      fit();
      // Width changes (window resize, sidebar) re-wrap the text.
      let lastWidth = el.clientWidth;
      const observer = new ResizeObserver(() => {
        if (el.clientWidth !== lastWidth) {
          lastWidth = el.clientWidth;
          fit();
        }
      });
      observer.observe(el);
      return () => observer.disconnect();
    }, [autoGrow, props.value]);

    return (
      <textarea
        ref={innerRef}
        className={cn(
          "w-full resize-y rounded-md border border-control bg-surface px-3 py-2 text-sm text-text-primary placeholder:text-text-secondary",
          "transition-colors hover:bg-surface-secondary focus-visible:bg-surface-secondary",
          "disabled:cursor-not-allowed disabled:opacity-50",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
          autoGrow && "resize-none overflow-y-hidden",
          className,
        )}
        style={autoGrow ? { resize: "none", ...style } : style}
        {...props}
      />
    );
  },
);
Textarea.displayName = "Textarea";
