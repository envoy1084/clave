import type { ReactNode } from "react";

import { Reveal } from "@/components/motion/reveal";
import { cn } from "@/lib/cn";

/** The page column. `wide` is for full-bleed moments (the hero and the bar above it), so they sit closer to the edges. */
export function Container({
  children,
  className,
  wide = false,
}: {
  children: ReactNode;
  className?: string;
  wide?: boolean;
}) {
  return (
    <div
      className={cn(
        "mx-auto w-full",
        wide ? "max-w-[1600px] px-5 sm:px-10 lg:px-14" : "max-w-[1160px] px-5 sm:px-8",
        className,
      )}
    >
      {children}
    </div>
  );
}

/** A page section: an anchor, a heading with a short lede, then its content. */
export function Section({
  id,
  title,
  lede,
  children,
  className,
  aside,
  tone = "light",
}: {
  id: string;
  title: ReactNode;
  lede?: ReactNode;
  children: ReactNode;
  className?: string;
  aside?: ReactNode;
  /** `dark` is a navy chapter: used for one section per page, never alternated by default. */
  tone?: "light" | "dark";
}) {
  return (
    <section
      id={id}
      aria-labelledby={`${id}-title`}
      className={cn("py-16 sm:py-24", tone === "dark" && "bg-navy text-paper", className)}
    >
      <Container>
        <Reveal className="grid gap-6 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
          <div className="max-w-[640px]">
            <h2
              id={`${id}-title`}
              className="serif-display text-[2.2rem] leading-[1.06] sm:text-[3rem]"
            >
              {title}
            </h2>
            {lede && (
              <div className="mt-4 text-[1rem] leading-relaxed opacity-80 sm:text-[1.05rem]">
                {lede}
              </div>
            )}
          </div>
          {aside}
        </Reveal>
        <div className="mt-12 sm:mt-14">{children}</div>
      </Container>
    </section>
  );
}
