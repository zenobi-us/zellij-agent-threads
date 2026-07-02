import { ServerCodeBlock } from 'fumadocs-ui/components/codeblock.rsc';

const planPreview = `
$ boxfiles plan workstation.yml

facts
  os.platform       linux
  user.name         kin
  shell.current     zsh

steps
  01 base.user          link
  02 devtools.starship  install
  03 devtools.zshrc     link
  04 devtools.fcegi     clone
  05 devtools.decfa     shell

result
  5 actions planned
  0 mutations executed`;

export async function PlanPreview() {
  return (
    <ServerCodeBlock
      code={planPreview}
      lang="bash"
      themes={{ light: 'github-dark', dark: 'github-dark' }}
      codeblock={{
        title: 'plan preview',
        className: 'border-rp-muted/30 bg-rp-base text-rp-text',
        viewportProps: {
          className: 'bg-rp-base text-rp-text',
        },
      }}
    />
  );
}


