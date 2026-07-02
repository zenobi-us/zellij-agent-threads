import "../app.css";
import { CtaHero } from "../components/CtaHero";
import { HeroAction } from "../components/HeroAction";
import { HeroActions } from "../components/HeroActions";
import { PlanPreview } from "../components/PlanPreview";
import { Site } from "../components/Site";
import { Section } from "../components/Section";
import { Logo } from "../components/Logo";
import { ReleaseVersion } from "../components/ReleaseVersion";

export default function Page() {
  return (
    <Site>
      <Section className="min-h-lvh items-center gap-12">
        <CtaHero
          tagline={
            <Logo
              className="self-center text-rp-overlay"
              suffix={<ReleaseVersion />}
            />
          }
          title="Agent session overview plugin for Zellij"
          subtitle="Agent Threads is a plugin for Zellij that provides an overview of all agent sessions, allowing you to easily monitor and manage your agents."
        >
          <HeroActions>
            <HeroAction primary asChild>
              <a href="/quickstart">Get started</a>
            </HeroAction>
            <HeroAction asChild>
              <a
                href="https://github.com/zenobi-us/zellij-agent-threads"
                target="_blank"
                rel="noopener noreferrer"
              >
                GitHub
              </a>
            </HeroAction>
          </HeroActions>
        </CtaHero>
        <PlanPreview />
      </Section>
    </Site>
  );
}
