import type { ReactNode } from "react";

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <header className="flex items-end justify-between gap-8 pt-6 pb-8">
      <div className="min-w-0">
        <h1 className="serif-display truncate text-[2.5rem] leading-[1.04]">{title}</h1>
        {description && <div className="mt-2 text-sm text-muted-foreground">{description}</div>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  );
}

export function Panel({
  title,
  action,
  children,
  className,
}: {
  title?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section
      className={`rounded-2xl bg-card shadow-[0_0_0_1px_var(--border),0_1px_2px_rgb(21_24_15/0.03)] ${className ?? ""}`}
    >
      {title && (
        <div className="flex items-center justify-between gap-4 px-5 pt-4 pb-1">
          <h2 className="text-sm font-medium">{title}</h2>
          {action}
        </div>
      )}
      {children}
    </section>
  );
}
