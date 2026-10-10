import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://cortex-ai.github.io',
  integrations: [
    starlight({
      title: 'Cortex',
      description: 'Open-source Rust runtime for autonomous AI workers',
      social: {
        github: 'https://github.com/x1-xh/cortex',
      },
      customCss: ['./src/styles/custom.css'],
      head: [
        {
          tag: 'link',
          attrs: {
            rel: 'preconnect',
            href: 'https://fonts.googleapis.com',
          },
        },
        {
          tag: 'link',
          attrs: {
            rel: 'preconnect',
            href: 'https://fonts.gstatic.com',
            crossorigin: '',
          },
        },
        {
          tag: 'link',
          attrs: {
            rel: 'stylesheet',
            href: 'https://fonts.googleapis.com/css2?family=Righteous&family=Bungee&family=Inter:wght@400;500;600;700&family=JetBrains+Mono:ital,wght@0,400;0,600;0,700;1,400&display=swap',
          },
        },
      ],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Quickstart', slug: 'getting-started/quickstart' },
            { label: 'Installation', slug: 'getting-started/installation' },
          ],
        },
        {
          label: 'Architecture',
          items: [
            { label: 'Overview', slug: 'architecture/overview' },
            { label: 'Runtime Lifecycle', slug: 'architecture/runtime' },
            { label: 'Persistent Agents', slug: 'architecture/agents' },
            { label: 'Sandbox & Boundaries', slug: 'architecture/sandbox' },
            { label: 'Scheduler Engine', slug: 'architecture/scheduler' },
            { label: 'Tool Registry', slug: 'architecture/tools' },
            { label: 'Execution Tracing & SQLite', slug: 'architecture/tracing' },
          ],
        },
        {
          label: 'User Guides',
          items: [
            { label: 'Autonomous Coding Agent', slug: 'guides/coding-agent' },
            { label: 'Terminal Control Plane (TUI)', slug: 'guides/tui' },
            { label: 'Multi-Agent Coordination', slug: 'guides/multi-agent' },
            { label: 'Model Context Protocol (MCP)', slug: 'guides/mcp' },
            { label: 'Benchmark Harness', slug: 'guides/benchmarking' },
            { label: 'Agent Skills Standard', slug: 'guides/skills' },
            { label: 'Cron Automation', slug: 'guides/cron' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'CLI Reference', slug: 'reference/cli' },
            { label: 'Agent Configuration', slug: 'reference/configuration' },
          ],
        },
        {
          label: 'Security & Project',
          items: [
            { label: 'Threat Model & Security Policy', slug: 'security/threat-model' },
            { label: 'GitHub Workflow', slug: 'development/github-workflow' },
            { label: 'Initial Issues', slug: 'development/initial-issues' },
            { label: 'Roadmap', slug: 'roadmap' },
          ],
        },
      ],
      tableOfContents: {
        minHeadingLevel: 2,
        maxHeadingLevel: 4,
      },
      pagination: true,
    }),
  ],
});
