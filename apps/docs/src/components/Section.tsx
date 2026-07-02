import classNames from 'classnames';
import type { HTMLAttributes, PropsWithChildren } from 'react';

export function Section(props: PropsWithChildren<HTMLAttributes<HTMLElement>>) {
  return (
    <section className={classNames("mx-auto grid w-full max-w-7xl gap-8 px-4 pb-16 sm:px-6 lg:grid-cols-[1fr_520px] lg:px-8 lg:pb-24", props.className)}>
      {props.children}
    </section>
  );
}

