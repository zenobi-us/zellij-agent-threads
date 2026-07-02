
import { defineDocs } from "fumadocs-mdx/config";
import { metaSchema, pageSchema } from "fumapress/adapters/mdx/schema";

// the config file for Fumadocs MDX, see https://fumadocs.dev/docs/mdx
export const docs = defineDocs({
  dir: "content",
  docs: {
    async: true,
    schema: pageSchema,
    postprocess: {
      includeProcessedMarkdown: true,
    },
  },
  meta: {
    schema: metaSchema,
  },
});

export const providerDocs = defineDocs({
  dir: "../../pkgs",
  docs: {
    files: ["provider-*/docs.md"],
    async: true,
    schema: pageSchema,
    postprocess: {
      includeProcessedMarkdown: true,
    },
  },
  meta: {
    schema: metaSchema,
  },
});

