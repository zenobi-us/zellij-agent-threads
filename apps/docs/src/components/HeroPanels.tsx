import classNames from "classnames";
import type { AboveTheFoldPanelsProps } from "./AboveTheFold";


export function HeroPanels(props: AboveTheFoldPanelsProps) {
  return (
    <section
      className={classNames(
        "relative grid w-full lg:grid-cols-2",
        "min-h-[clamp(48svh,60svh,65svh)]"
      )}
    >
      <div
        className={classNames(
          "flex flex-col justify-end gap-12 sm:gap-14 lg:gap-16",
          "border-b border-rp-muted/30 lg:border-b-0 lg:border-r",
          "bg-rp-surface text-rp-text",
          "p-6 sm:p-8 lg:p-12"
        )}
      >
        {props.left}
      </div>

      <div
        className={classNames(
          "flex flex-col justify-end gap-12 sm:gap-14 lg:gap-16",
          "flex-col-reverse lg:flex-col",
          "bg-rp-foam text-rp-base",
          "p-6 sm:p-8 lg:p-12"
        )}
      >
        {props.right}
      </div>
    </section>
  );
}

