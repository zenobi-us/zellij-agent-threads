import Type from "typebox";
import Value from "typebox/value";
import { useCallback, useEffect, useReducer } from "react";

export const ReleasePackageSchema = Type.Object({
  path: Type.String(),
  name: Type.String(),
  version: Type.String(),
  sha: Type.String(),
});

export const ReleaseVersionsSchema = Type.Object({
  generatedAt: Type.String(),
  repository: Type.String(),
  ref: Type.String(),
  sha: Type.String(),
  channel: Type.Union([Type.String(), Type.Null()]),
  packages: Type.Array(ReleasePackageSchema),
});

export type ReleaseVersions = Type.Static<typeof ReleaseVersionsSchema>;

export function parseReleaseVersions(data: unknown): ReleaseVersions {
  return Value.Parse(ReleaseVersionsSchema, data);
}

type LoadingState = { readonly status: "loading"; };
type LoadedState = { readonly status: "loaded"; readonly versions: readonly string[]; readonly manifest: ReleaseVersions; };
type ErrorState = { readonly status: "error"; readonly error: Error; };
type State = LoadingState | LoadedState | ErrorState;

type LoadAction = { readonly type: "load"; };
type LoadedAction = { readonly type: "loaded"; readonly manifest: ReleaseVersions; };
type ErrorAction = { readonly type: "error"; readonly error: Error; };
type Action = LoadAction | LoadedAction | ErrorAction;

const reducer = (state: State, action: Action): State => {
  switch (action.type) {
    case "load":
      return { status: "loading" };
    case "loaded":
      return {
        status: "loaded",
        versions: action.manifest.packages.map((releasePackage) => releasePackage.version),
        manifest: action.manifest,
      };
    case "error":
      return { status: "error", error: action.error };
    default:
      return state;
  }
};

/**
 * A hook that contains a useReducer state machine that goes through:
 * - loading, loaded, error states for fetching the release versions of boxfiles.
 */
export function useReleaseVersions() {
  const [state, dispatch] = useReducer(reducer, { status: "loading" });

  const load = useCallback(() => {
    dispatch({ type: "load" });

    fetch("/releases.json")
      .then((response) => response.json())
      .then((data: unknown) => {
        dispatch({ type: "loaded", manifest: parseReleaseVersions(data) });
      })
      .catch((error: unknown) => {
        dispatch({ type: "error", error: error instanceof Error ? error : new Error(String(error)) });
      });
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  return { ...state, load };
}
