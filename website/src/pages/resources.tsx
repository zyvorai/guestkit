import type {ReactNode} from 'react';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import Link from '@docusaurus/Link';
import styles from './resources.module.css';

type Asset = {
  title: string;
  blurb: string;
  meta: string;
  links: {label: string; to: string}[];
};

const ASSETS: Asset[] = [
  {
    title: 'CLI, TUI and QEMU runtime',
    meta: 'Linux · Apache-2.0',
    blurb:
      'guestkit, guestctl (TUI) and guestkit-qemu as release tarballs on GitHub, with checksums. Needs qemu-img, losetup and qemu-nbd on the host.',
    links: [
      {label: 'Latest release', to: 'https://github.com/zyvorai/guestkit/releases/latest'},
      {label: 'Getting started', to: '/docs/user-guides/getting-started'},
    ],
  },
  {
    title: 'Python bindings',
    meta: 'PyPI · zyvor-guestkit',
    blurb:
      'The same assurance engine as the CLI: run_doctor and run_migrate_repair, used by the h2kvm offline fixer. Install with pip; import stays "guestkit".',
    links: [
      {label: 'PyPI', to: 'https://pypi.org/project/zyvor-guestkit/'},
      {label: 'Python guide', to: '/docs/user-guides/python-bindings'},
    ],
  },
  {
    title: 'Web console and worker images',
    meta: 'GHCR · no login required',
    blurb:
      'zyvor-ui, zyvor-api and guestkit-worker under ghcr.io/zyvorai. Eval stack via docker compose; Helm chart for clusters.',
    links: [
      {label: 'GHCR packages', to: 'https://github.com/orgs/zyvorai/packages'},
      {label: 'Docker guide', to: '/docs/guides/DOCKER'},
    ],
  },
  {
    title: 'CI gate — GitHub Action',
    meta: 'uses: zyvorai/guestkit@v1',
    blurb:
      'Fail a pipeline when a disk scores below your threshold. Same score as the CLI, no install step.',
    links: [
      {label: 'Action on GitHub', to: 'https://github.com/zyvorai/guestkit'},
      {label: 'Passport CI gate runbook', to: '/docs/devops/passport-ci-gate'},
    ],
  },
  {
    title: '30-day Enterprise trial',
    meta: 'Binary · trial token included',
    blurb:
      'Try the control plane — Command Center, Passport Authority, Migration Factory — before you buy. The open-source engine is not locked.',
    links: [
      {label: 'Trial releases', to: 'https://github.com/zyvorai/guestkit/releases?q=enterprise-trial'},
      {label: 'Install guide', to: '/docs/enterprise-trial-install'},
      {label: 'Open source vs Enterprise', to: '/docs/ce-vs-enterprise'},
    ],
  },
  {
    title: 'Guides and runbooks',
    meta: 'Docs · wiki',
    blurb:
      'DevOps runbooks (Passport gate, repair worker, fleet, cutover weekend), the feature guide, and the operator wiki.',
    links: [
      {label: 'DevOps runbooks', to: '/docs/devops/'},
      {label: 'Feature guide', to: '/docs/guestkit-user-feature-guide'},
      {label: 'Wiki', to: 'https://github.com/zyvorai/guestkit/wiki'},
    ],
  },
];

function AssetCard({asset}: {asset: Asset}) {
  return (
    <article className={styles.card}>
      <Heading as="h2" className={styles.cardTitle}>
        {asset.title}
      </Heading>
      <p className={styles.meta}>{asset.meta}</p>
      <p className={styles.blurb}>{asset.blurb}</p>
      <div className={styles.actions}>
        {asset.links.map((l) => (
          <Link key={l.to} className="button button--primary button--sm" to={l.to}>
            {l.label}
          </Link>
        ))}
      </div>
    </article>
  );
}

export default function Resources(): ReactNode {
  return (
    <Layout
      title="Resources"
      description="Where to get GuestKit: release binaries, PyPI, container images, the GitHub Action and the Enterprise trial.">
      <main className="container margin-vert--lg">
        <header className={styles.header}>
          <Heading as="h1">Resources</Heading>
          <p className={styles.lead}>
            Everything you can install or pull today, and where its docs live.
            New here? Start with the{' '}
            <Link to="/docs/user-guides/getting-started">getting started guide</Link>.
          </p>
        </header>
        <div className={styles.grid}>
          {ASSETS.map((a) => (
            <AssetCard key={a.title} asset={a} />
          ))}
        </div>
        <p className={styles.note}>
          Questions? <a href="mailto:sales@zyvor.dev">sales@zyvor.dev</a>
          {' · '}
          <a href="https://zyvor.dev/guestkit?utm_source=github&utm_medium=guestkit" target="_blank" rel="noreferrer">
            zyvor.dev/guestkit
          </a>
        </p>
      </main>
    </Layout>
  );
}
