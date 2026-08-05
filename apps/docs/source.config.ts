
import { defineConfig, defineDocs } from "fumadocs-mdx/config";
import { metaSchema, pageSchema } from "fumapress/adapters/mdx/schema";

export default defineConfig({
  mdxOptions: {
    remarkPlugins: [remarkMermaidCodeblocks],
  },
});

function remarkMermaidCodeblocks() {
  return (tree: unknown) => replaceMermaidCodeblocks(tree);
}

function replaceMermaidCodeblocks(node: unknown): void {
  if (!node || typeof node !== "object") return;

  const parent = node as { children?: unknown[] };
  if (!Array.isArray(parent.children)) return;

  parent.children = parent.children.map((child) => {
    if (isMermaidCodeblock(child)) {
      return {
        type: "mdxJsxFlowElement",
        name: "Mermaid",
        attributes: [{ type: "mdxJsxAttribute", name: "chart", value: child.value }],
        children: [],
      };
    }

    replaceMermaidCodeblocks(child);
    return child;
  });
}

function isMermaidCodeblock(node: unknown): node is { type: "code"; lang: string; value: string } {
  if (!node || typeof node !== "object") return false;
  const code = node as { type?: unknown; lang?: unknown; value?: unknown };
  return code.type === "code" && code.lang === "mermaid" && typeof code.value === "string";
}

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

