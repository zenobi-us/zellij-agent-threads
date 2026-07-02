import { File, Files, Folder } from 'fumadocs-ui/components/files';

export function WorkspaceFilesPreview() {
  return (
    <Files className="border-rp-muted/30 bg-rp-base text-rp-text">
      <Folder name="my-dotfiles-repo" defaultOpen>
        <File name=".boxfilesrc" />
        <File name="base.yml" />
        <File name="workstation.yml" />
        <Folder name="files" defaultOpen>
          <File name="zshrc" />
        </Folder>
      </Folder>
    </Files>
  );
}
