import '../app.css';
import { CtaHero } from '../components/CtaHero';
import { HeroAction } from '../components/HeroAction';
import { HeroActions } from '../components/HeroActions';
import { PlanPreview } from '../components/PlanPreview';
import { Site } from '../components/Site';
import { Section } from '../components/Section';
import { CopyTextToClipboard } from '../components/CopyTextToClipboard';
import { Logo } from '../components/Logo';
import { ReleaseVersion } from '../components/ReleaseVersion';

export default function Page() {


  return (
    <Site>
      <Section className="min-h-lvh items-center gap-12">
        <CtaHero

          tagline={
            <Logo className="self-center text-rp-overlay" suffix={<ReleaseVersion />} />
          }
          title="Provision machines without ceremony."
          subtitle="Boxfiles turns manifests of steps and facts into a idompotent plan you can inspect before it touches a workstation."
        >
          <HeroActions>
            <HeroAction asChild><CopyTextToClipboard text="mise use -g github:boxfiles/boxfiles" /></HeroAction>
            <HeroAction primary asChild><a href="/quickstart">Get started</a></HeroAction>
            <HeroAction asChild><a href="https://github.com/boxfiles/boxfiles" target="_blank" rel="noopener noreferrer">GitHub</a></HeroAction>
          </HeroActions>
        </CtaHero>
        <PlanPreview />
      </Section>
    </Site>
  );
}


