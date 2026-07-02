import classNames from 'classnames';
import type { PropsWithChildren, HTMLAttributes } from 'react';


export function Hero(props: PropsWithChildren<HTMLAttributes<'div'>>) {
  return (
    <div className={classNames("flex max-w-2xl flex-col justify-center gap-8", props.className)}>
      {props.children}
    </div>
  );
}

