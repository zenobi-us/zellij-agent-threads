import type { PropsWithChildren } from 'react';
import '../app.css';

export function Site(props: PropsWithChildren) {

  return (
    <main className="flex min-h-screen flex-col  bg-rp-base text-rp-text">
      {props.children}
    </main>
  )
}
