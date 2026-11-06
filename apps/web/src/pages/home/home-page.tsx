import { NextPage } from "@/components/ui/next-page";

import { Exposure } from "./exposure";
import { Hero } from "./hero";
import { Numbers } from "./numbers";
import { Roadmap } from "./roadmap";
import { SystemTour } from "./system-tour";
import { Thresholds } from "./thresholds";

export function HomePage() {
  return (
    <>
      <Hero />
      <Thresholds />
      <SystemTour />
      <Exposure />
      <Numbers />
      <Roadmap />
      <NextPage
        to="/spending"
        label="How spending works"
        teaser="Follow a payment from proposal to proof: approvals, the auth witness and the entrypoint check."
      />
    </>
  );
}
