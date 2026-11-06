import { useEffect, useRef, useState } from "react";

import { useLocation } from "@tanstack/react-router";

import { AnimatePresence, motion } from "motion/react";

import { step as stepMotion } from "@/lib/motion";
import { useAppState } from "@/lib/state";

import { OnboardingFrame } from "./frame";
import { Initializing, ProfileForm } from "./setup";
import { Welcome } from "./welcome";

type Step = "welcome" | "profile" | "init";
const ORDER: Step[] = ["welcome", "profile", "init"];

/**
 * Onboarding as one screen: /welcome and /setup render this, and the steps swap inside a frame that stays mounted.
 * The direction of each swap follows the step order.
 */
export function Onboarding() {
  const { pathname } = useLocation();
  const { data: state } = useAppState();
  const [created, setCreated] = useState(false);
  const current: Step = pathname.startsWith("/setup")
    ? created || state?.profile
      ? "init"
      : "profile"
    : "welcome";
  const previous = useRef<Step>(current);
  const direction = ORDER.indexOf(current) >= ORDER.indexOf(previous.current) ? 1 : -1;
  useEffect(() => {
    previous.current = current;
  }, [current]);
  return (
    <OnboardingFrame step={ORDER.indexOf(current) + 1} steps={ORDER.length}>
      <AnimatePresence mode="popLayout" initial={false} custom={direction}>
        <motion.div
          key={current}
          custom={direction}
          variants={stepMotion}
          initial="enter"
          animate="center"
          exit="exit"
          className="w-full"
        >
          {current === "welcome" && <Welcome />}
          {current === "profile" && <ProfileForm onCreated={() => setCreated(true)} />}
          {current === "init" && <Initializing />}
        </motion.div>
      </AnimatePresence>
    </OnboardingFrame>
  );
}
