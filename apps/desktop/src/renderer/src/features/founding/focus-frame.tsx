import type { ReactNode } from "react";

import { Link } from "@tanstack/react-router";

import { ArrowLeftIcon } from "lucide-react";
import { motion } from "motion/react";

import { Wordmark } from "@/components/brand/mark";
import { EASE_OUT } from "@/lib/motion";

/** A single-task screen: a quiet top bar with a way back, and the task centred below. */
export function FocusFrame({
  back,
  title,
  children,
  wide = false,
}: {
  back?: "/start" | "/";
  title: string;
  children: ReactNode;
  wide?: boolean;
}) {
  return (
    <div className="flex h-screen flex-col overflow-hidden bg-paper">
      <header className="app-drag grid h-14 shrink-0 grid-cols-[1fr_auto_1fr] items-center pr-5 pl-24">
        <div>
          {back && (
            <Link
              to={back}
              className="app-no-drag inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-sm text-muted-foreground transition-colors duration-150 hover-fine:bg-accent hover-fine:text-foreground"
            >
              <ArrowLeftIcon className="size-4" /> Back
            </Link>
          )}
        </div>
        <p className="text-sm font-medium text-muted-foreground">{title}</p>
        <div className="flex justify-end">
          <Wordmark className="text-sm text-muted-foreground" />
        </div>
      </header>
      <main className="quiet-scroll flex-1 overflow-y-auto px-10 pb-16">
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.32, ease: EASE_OUT }}
          className={wide ? "mx-auto max-w-[980px] pt-10" : "mx-auto max-w-[860px] pt-12"}
        >
          {children}
        </motion.div>
      </main>
    </div>
  );
}
