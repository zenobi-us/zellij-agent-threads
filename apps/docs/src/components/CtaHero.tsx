import type { HTMLAttributes, PropsWithChildren, ReactNode } from 'react';
import { Hero } from './Hero';

export function CtaHero(props: PropsWithChildren<{
  readonly title: ReactNode;
  readonly subtitle?: ReactNode;
  readonly tagline?: ReactNode
} & HTMLAttributes<HTMLDivElement>>) {
  const { title, subtitle, tagline, children, ...rest } = props;
  return (
    <Hero {...rest}>
      <div className="space-y-5">
        {tagline}
        <h2 className="text-5xl font-semibold leading-[0.95] tracking-tighter text-rp-text sm:text-7xl">
          {title}
        </h2>
        <p className="max-w-xl text-lg leading-8 text-rp-subtle">
          {subtitle}
        </p>
      </div>
      {children}
    </Hero>
  );
}


