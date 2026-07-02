import type { PropsWithChildren } from 'react';

export function HeroActions(props: PropsWithChildren) {
  return (
    <div className="flex flex-col gap-3 sm:flex-row">
      {props.children}
    </div>
  );
}
