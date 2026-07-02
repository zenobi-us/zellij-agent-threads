import type { ReactNode } from "react"
import classNames from "classnames"
import { HeroPanels } from "./HeroPanels"

export type AboveTheFoldPanelsProps = {
  readonly left: ReactNode
  readonly right: ReactNode
}

export function AboveTheFold() {
  return (
    <HeroPanels
      left={(
        <>
          <h1
            className={classNames(
              "max-w-3xl font-semibold leading-none tracking-tighter",
              "text-[clamp(1.5rem,8vw,3rem)] xl:text-[clamp(3rem,4vw,6rem)]",
            )}
          >
            Shell scripts, tribal notes, mystery state.
          </h1>
          <p
            className={classNames(
              "ml-auto font-medium uppercase tracking-[0.16em] text-rp-subtle",
              "text-xs",
            )}
          >
            Before
          </p>
        </>
      )}
      right={(
        <>
          <h2
            className={classNames(
              "max-w-3xl font-semibold leading-none tracking-tighter",
              "text-[clamp(1.5rem,8vw,3rem)] xl:text-[clamp(3rem,4vw,6rem)]",
            )}
          >
            Manifests, facts, typed plans.
          </h2>
          <p
            className={classNames(
              "self-end lg:self-auto",
              "font-medium uppercase tracking-[0.16em] text-rp-overlay",
              "text-xs",
            )}
          >
            After
          </p>
        </>
      )}
    />
  )
}
