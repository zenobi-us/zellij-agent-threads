
import { defineConfig } from "fumapress";
import { llmsPlugin } from "fumapress/plugins/llms.txt";
import { flexsearchPlugin } from "fumapress/plugins/flexsearch";
import { fumadocsMdx } from "fumapress/adapters/mdx";
import { update } from "fumadocs-core/source";
// don't worry if this file is missing, we will run the dev command later to generate this file
import { docs, providerDocs } from "./.source/server";

const docsSource = update(docs.toFumadocsSource())
  .page((page) => {
    return page;
  })
  .build();

const providerSource = update(providerDocs.toFumadocsSource())
  .files((files) => {

    // filter out provider folders, only want the docs.md in them.
    return files.filter((file) => {
      return file.path.endsWith("docs.md");
    })
  })
  .page((page) => {
    const name = page.path.split("/")[0]?.replace(/^provider-/, "") ?? page.path;
    const slugs = ['plugins', 'builtin', name]

    return {
      ...page,
      slugs,
      path: slugs.join("/"),
      data: {
        ...page.data,
        info: {
          ...page.data.info,
          path: `/${slugs.join("/")}`,
        },
        title: name
      }
    };
  })
  .build();


export default defineConfig({
  content: {
    docs: docsSource,
    providerDocs: providerSource,
  },
  mode: "static",
  site: {
    name: "Boxfiles",

  },
  meta: {
    root() {
      return (
        <>
          <link rel="preconnect" href="https://fonts.googleapis.com" />
          <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="" />
          <link
            href="https://fonts.googleapis.com/css2?family=Geist+Mono:wght@100..900&family=Geist:wght@100..900&display=swap"
            rel="stylesheet"
          />
        </>
      )
    },
    page() {
      return (
        <></>
      )
    }
  },

})
  // extend via plugins
  .plugins(flexsearchPlugin(), llmsPlugin())
  // use different content sources
  .adapters(fumadocsMdx());

