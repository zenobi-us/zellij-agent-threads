'use client';

import { useReleaseVersions } from './useReleaseVersions';

export function ReleaseVersion() {
  const releases = useReleaseVersions();
  const cliVersion = releases.status === 'loaded'
    ? releases.manifest.packages.find((releasePackage) => releasePackage.name === '@boxfiles/cli')?.version
    : undefined;

  return (
    <>@<span>{cliVersion ?? 'x.x.x'}</span></>
  );

}

