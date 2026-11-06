import type { ReactNode } from "react";

import { Reveal } from "@/components/motion/reveal";
import { cn } from "@/lib/cn";

export function Container({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cn("mx-auto w-full max-w-[1160px] px-5 sm:px-8", className)}>{children}</div>
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
}: {
  id: string;
  title: ReactNode;
  lede?: ReactNode;
  children: ReactNode;
  className?: string;
  aside?: ReactNode;
}) {
  return (
    <section id={id} aria-labelledby={`${id}-title`} className={cn("py-16 sm:py-24", className)}>
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
