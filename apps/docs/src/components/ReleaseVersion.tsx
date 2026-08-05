'use client';

import { useReleaseVersions } from './useReleaseVersions';

export function ReleaseVersion() {
  const releases = useReleaseVersions();
  const cliVersion = releases.status === 'loaded'
    ? releases.manifest.packages.find((releasePackage) => releasePackage.name === 'agent-threads')?.version
    : undefined;

  if (!cliVersion) return null;

  return (
    <>@<span>{cliVersion}</span></>
  );

}

