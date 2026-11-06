import { useNavigate } from "@tanstack/react-router";

import { ArrowRightIcon, EyeOffIcon, KeyRoundIcon, UsersRoundIcon } from "lucide-react";
import { motion } from "motion/react";

import { Button } from "@/components/ui/button";
import { rise } from "@/lib/motion";

import { OnboardingFrame } from "./frame";

const POINTS = [
  {
    icon: UsersRoundIcon,
    title: "Every payment needs your co-signers",
    body: "You choose how many approvals it takes. No one moves funds alone.",
  },
  {
    icon: EyeOffIcon,
    title: "Private, even from each other's devices",
    body: "Balances and history are readable only when enough members' keepers work together.",
  },
  {
    icon: KeyRoundIcon,
    title: "Your keys stay on this device",
    body: "Your signing key is sealed in your system keychain and never leaves it.",
  },
];

export function Welcome() {
  const navigate = useNavigate();
  return (
    <OnboardingFrame seed={21} caption="A vault that no one holds the key to alone.">
      <motion.div
        initial="hidden"
        animate="shown"
        variants={{ shown: { transition: { staggerChildren: 0.06, delayChildren: 0.05 } } }}
      >
        <motion.h1 variants={rise} className="serif-display text-[2.9rem] leading-[1.02] text-ink">
          Shared custody,
          <br />
          private by default.
        </motion.h1>
        <motion.p
          variants={rise}
          className="mt-4 max-w-[38ch] text-[0.95rem] leading-relaxed text-muted-foreground"
        >
          Tecpan is a multisig vault on Aztec. Set it up with your co-founders, approve payments
          together, and keep everything you hold out of public view.
        </motion.p>
        <motion.ul variants={rise} className="mt-9 space-y-5">
          {POINTS.map(({ icon: Icon, title, body }) => (
            <li key={title} className="flex gap-3.5">
              <span className="mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg bg-paper-2 text-ink shadow-[inset_0_0_0_1px_var(--border)]">
                <Icon className="size-4" strokeWidth={1.75} />
              </span>
              <div>
                <p className="text-sm font-medium">{title}</p>
                <p className="mt-0.5 text-[0.825rem] leading-snug text-muted-foreground">{body}</p>
              </div>
            </li>
          ))}
        </motion.ul>
        <motion.div variants={rise} className="mt-10 flex items-center gap-4">
          <Button
            size="xl"
            onClick={() => void navigate({ to: "/setup" })}
            data-testid="welcome-start"
          >
            Set up this device
            <ArrowRightIcon data-icon="inline-end" />
          </Button>
          <p className="text-xs text-muted-foreground">
            About a minute. Nothing is shared until you invite someone.
          </p>
        </motion.div>
      </motion.div>
    </OnboardingFrame>
  );
}
