import type { ExtensionContext } from "@earendil-works/pi-coding-agent";

export type PiCtx = ExtensionContext;
export type StatusValues = Record<string, string | undefined>;

/**
 * Owns Pi footer rendering so transport code can report progress without knowing
 * how the user chose to format the status line.
 */
export class StatusWidget {
  lastStatus = "init";

  constructor(
    private readonly key: string,
    private readonly template: string,
  ) {}

  /**
   * Stores the latest status for diagnostics and renders through the configured
   * template. Stale contexts happen during session replacement, so callers should
   * not need their own guard around every status update.
   */
  update(ctx: PiCtx, status: string, values: StatusValues): void {
    this.lastStatus = status;
    try {
      this.render(ctx, { ...values, status });
    } catch {
      // ponytail: ctx can go stale after session replacement; Zellij publish must not crash Pi.
    }
  }

  /**
   * Renders one footer value for Pi. Kept separate from `update` so tests and
   * future commands can render arbitrary snapshots without mutating diagnostics.
   */
  render(ctx: PiCtx, values: StatusValues): void {
    if (!ctx.hasUI) return;
    ctx.ui.setStatus(this.key, this.interpolate(values));
  }

  /**
   * Uses tiny token replacement instead of a template engine because status
   * interpolation only needs named scalar values.
   */
  private interpolate(values: StatusValues): string {
    return this.template.replace(/\{(\w+)\}/g, (_match, key: string) => values[key] ?? "");
  }
}
