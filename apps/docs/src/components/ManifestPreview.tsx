import { ServerCodeBlock } from "fumadocs-ui/components/codeblock.rsc";

const manifestPreview = `---
dependsOn:
  - base

steps:
  - id: starship
    uses: install
    with:
      package: starship

  - id: zshrc
    uses: link
    with:
      from: zshrc
      to: ~/.zshrc

  - uses: clone
    with:
      repo: https://github.com/example/nvim.git
      to: ~/.config/nvim

  - uses: shell
    with:
      command: nvim --headless '+Lazy! sync' +qa`

export async function ManifestPreview() {
  return (
    <ServerCodeBlock
      code={manifestPreview}
      lang="yaml"
      themes={{ light: 'github-dark', dark: 'github-dark' }}
      codeblock={{
        title: 'workstation.yml',
        className: 'border-rp-muted/30 bg-rp-base text-rp-text',
        viewportProps: {
          className: 'bg-rp-base text-rp-text',
        },
      }}
    />
  );
}

