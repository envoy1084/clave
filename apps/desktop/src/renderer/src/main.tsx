import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";

import { MotionConfig } from "motion/react";
import { Toaster } from "sonner";

import { TooltipProvider } from "@/components/ui/tooltip";
import { queryClient } from "@/lib/state";

import { router } from "./router";

import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <MotionConfig reducedMotion="user">
        <TooltipProvider>
          <RouterProvider router={router} />
          <Toaster
            position="bottom-right"
            gap={8}
            toastOptions={{
              classNames: {
                toast:
                  "!rounded-xl !border-0 !bg-ink !text-paper !shadow-[0_16px_40px_-12px_rgb(21_24_15/0.45)] !font-sans",
                description: "!text-paper/70",
                actionButton: "!bg-signal !text-ink !rounded-md",
              },
            }}
          />
        </TooltipProvider>
      </MotionConfig>
    </QueryClientProvider>
  </StrictMode>,
);
