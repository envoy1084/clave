import { Section } from "@/components/ui/section";
import { cn } from "@/lib/cn";
import { STATUS } from "@/lib/content";

export function Roadmap() {
  return (
    <Section
      id="status"
      title={
        <>
          Where v0 <em className="serif-em">stands.</em>
        </>
      }
      lede="Ticked items are built and covered by tests in the repository. The rest come next, in this order."
    >
      <ol className="grid gap-x-10 sm:grid-cols-2">
        {STATUS.map((item) => (
          <li
            key={item.name}
            className="grid grid-cols-[1.75rem_1fr] gap-x-3 border-t border-line py-4"
          >
            <span
              aria-hidden
              className={cn(
                "mt-0.5 grid size-5 place-items-center rounded-full text-[0.68rem] font-semibold",
                item.done ? "bg-olive text-paper" : "shadow-[inset_0_0_0_1.5px_var(--color-muted)]",
              )}
            >
              {item.done ? "✓" : ""}
            </span>
            <div>
              <p className="text-[0.95rem] font-medium">
                {item.name}
                <span className="sr-only">{item.done ? ", done" : ", next"}</span>
              </p>
              <p className="mt-0.5 text-[0.86rem] text-ink-soft">{item.detail}</p>
            </div>
          </li>
        ))}
      </ol>
    </Section>
  );
}
