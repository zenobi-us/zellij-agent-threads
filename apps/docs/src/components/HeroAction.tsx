import type { ComponentPropsWithoutRef, PropsWithChildren } from 'react';
import classnames from 'classnames';
import { Slot } from '@radix-ui/react-slot';

type HeroActionProps = PropsWithChildren<
  Omit<ComponentPropsWithoutRef<'a'>, 'className'> & {
    readonly className?: string;
    readonly primary?: boolean;
    readonly asChild?: boolean;
  }
>;

export function HeroAction(props: HeroActionProps) {
  const Comp = props.asChild ? Slot : 'a';
  const { primary: _primary, asChild: _asChild, className, ...rest } = props;

  return (
    <Comp
      data-variant={props.primary ? 'primary' : 'secondary'}
      className={classnames(
        'inline-flex min-h-11 items-center justify-center rounded-lg px-5 text-sm font-semibold transition-colors',
        'cursor-pointer',
        'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rp-foam',
        'data-[variant=primary]:bg-rp-foam data-[variant=primary]:text-rp-base data-[variant=primary]:hover:bg-rp-gold',
        'data-[variant=secondary]:border data-[variant=secondary]:border-rp-muted/40 data-[variant=secondary]:text-rp-text data-[variant=secondary]:hover:bg-rp-surface',
        className,
      )}
      {...rest}
    >
      {props.children}
    </Comp>
  );
}

