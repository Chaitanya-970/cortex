import { defineCollection } from 'astro:content';
import { docsSchema } from '@astrojs/starlight/schema';
import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

function cortexDocsLoader(): any {
  return {
    name: 'cortex-docs-loader',
    load: async ({ store, logger, parseData, generateDigest, entryTypes, config, watcher }: any) => {
      logger.info('Loading documentation files from ../docs...');
      store.clear();

      const docsDir = path.resolve(process.cwd(), '../docs');
      const mdEntryType = entryTypes.get('.md');
      const render = await mdEntryType.getRenderFunction(config);

      async function walkDir(dir: string, base: string): Promise<string[]> {
        const entries = await fs.readdir(dir, { withFileTypes: true });
        const files: string[] = [];
        for (const entry of entries) {
          const fullPath = path.join(dir, entry.name);
          const relPath = path.relative(base, fullPath);
          if (entry.isDirectory()) {
            files.push(...(await walkDir(fullPath, base)));
          } else if (entry.isFile() && entry.name.endsWith('.md')) {
            files.push(relPath);
          }
        }
        return files;
      }

      const files = await walkDir(docsDir, docsDir);
      for (const relFile of files) {
        const fullPath = path.join(docsDir, relFile);
        const relToSite = path.relative(process.cwd(), fullPath).replace(/\\/g, '/');
        let contents = await fs.readFile(fullPath, 'utf-8');

        // Transform relative markdown links to clean doc routes
        // E.g. [tracing](../architecture/tracing.md) -> [tracing](/architecture/tracing/)
        // E.g. [tui](../guides/tui.md) -> [tui](/guides/tui/)
        contents = contents.replace(/\]\(\.\.\/([a-zA-Z0-9_\-\/]+)\.md\)/g, '](/$1/)');
        contents = contents.replace(/\]\((\.\/[a-zA-Z0-9_\-\/]+)\.md\)/g, '](/$1/)');
        // Link to repository files for crates/...
        contents = contents.replace(
          /\]\(crates\/([^)]+)\)/g,
          '](https://github.com/x1-xh/cortex/blob/main/crates/$1)'
        );

        const headingMatch = contents.match(/^#\s+(.+)$/m);
        const title = headingMatch ? headingMatch[1].trim() : path.basename(relFile, '.md');

        let description = '';
        const lines = contents.split('\n');
        for (let i = 0; i < lines.length; i++) {
          const line = lines[i].trim();
          if (line.startsWith('#') || line.startsWith('>') || line.startsWith('```') || line === '') {
            continue;
          }
          description = line.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1');
          if (description.length > 160) {
            description = description.slice(0, 157) + '...';
          }
          break;
        }

        let id = relFile.replace(/\.md$/, '').replace(/\\/g, '/');
        if (id.endsWith('/index')) {
          id = id.slice(0, -6);
        }

        const fileUrl = pathToFileURL(fullPath);
        const { body, data } = await mdEntryType.getEntryInfo({
          contents,
          fileUrl,
        });

        if (!data.title) {
          data.title = title;
        }
        if (!data.description && description) {
          data.description = description;
        }

        const digest = generateDigest(contents);
        const parsedData = await parseData({
          id,
          data,
          filePath: relToSite,
        });

        const rendered = await render({
          id,
          data: parsedData,
          body,
          filePath: fullPath,
          digest,
        });

        store.set({
          id,
          data: parsedData,
          body,
          filePath: relToSite,
          digest,
          rendered,
          assetImports: rendered?.metadata?.imagePaths,
        });
      }

      if (watcher) {
        watcher.add(docsDir);
      }
    },
  };
}

export const collections = {
  docs: defineCollection({ loader: cortexDocsLoader(), schema: docsSchema() }),
};
