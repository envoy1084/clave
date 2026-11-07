import { Exposure } from "./exposure";
import { Hero } from "./hero";
import { Numbers } from "./numbers";
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
    </>
  );
}
