import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

import "./iso.css";

/** The surface a figure sits on, in the figure's light theme. Words stay in the page, not in the drawing. */
export function FigureCard({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <figure className={cn("iso", className)} data-theme="light">
      {children}
    </figure>
  );
}
