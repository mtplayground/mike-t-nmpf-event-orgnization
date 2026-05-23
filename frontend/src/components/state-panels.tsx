import { LoaderCircle, type LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from '@/lib/utils';

type StatusPanelProps = {
  title: string;
  body: string;
  tone?: 'default' | 'success' | 'error';
};

export function StatusPanel({
  body,
  title,
  tone = 'default',
}: StatusPanelProps) {
  return (
    <div
      className={cn(
        'rounded-2xl border p-4 text-sm leading-6',
        tone === 'success' &&
          'border-emerald-300/60 bg-emerald-500/10 text-emerald-900 dark:text-emerald-100',
        tone === 'error' &&
          'border-rose-300/60 bg-rose-500/10 text-rose-900 dark:text-rose-100',
        tone === 'default' &&
          'border-border/70 bg-background/80 text-muted-foreground',
      )}
    >
      <p className="font-medium">{title}</p>
      <p className="mt-1">{body}</p>
    </div>
  );
}

type EmptyStatePanelProps = {
  title: string;
  body: string;
  icon?: LucideIcon;
  action?: ReactNode;
};

export function EmptyStatePanel({
  action,
  body,
  icon: Icon,
  title,
}: EmptyStatePanelProps) {
  return (
    <div className="rounded-2xl border border-dashed border-border/70 bg-background/70 p-5">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start">
        {Icon ? (
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-secondary text-muted-foreground">
            <Icon className="h-5 w-5" />
          </div>
        ) : null}
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium text-foreground">{title}</p>
          <p className="mt-1 text-sm leading-6 text-muted-foreground">{body}</p>
          {action ? <div className="mt-4">{action}</div> : null}
        </div>
      </div>
    </div>
  );
}

type LoadingStateProps = {
  title: string;
  body: string;
};

export function LoadingState({ body, title }: LoadingStateProps) {
  return (
    <div className="rounded-2xl border border-border/70 bg-background/80 p-5 text-sm text-muted-foreground">
      <div className="flex items-center gap-3">
        <LoaderCircle className="h-5 w-5 animate-spin text-primary" />
        <div>
          <p className="font-medium text-foreground">{title}</p>
          <p className="mt-1 leading-6">{body}</p>
        </div>
      </div>
    </div>
  );
}
