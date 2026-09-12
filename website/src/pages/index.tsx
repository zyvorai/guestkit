import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import FeatureHighlights from '@site/src/components/FeatureHighlights';
import Reveal from '@site/src/components/Reveal';

import styles from './index.module.css';

function HomepageHeader() {
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <div className={clsx(styles.heroGridSingle, 'text--center')}>
          <Heading as="h1" className="hero__title">
            Offline VM intelligence.
          </Heading>
          <p className="hero__subtitle">
            Score boot readiness before power-on, repair disks offline, and
            certify cutover with a signed Passport — no appliance daemon, no
            "just try it and hope." GuestKit reads the disk while the guest
            is off and emits a reviewable fix plan.
          </p>
          <div className={styles.buttons}>
            <Link
              className="button button--secondary button--lg"
              to="/docs/user-guides/getting-started">
              Get Started
            </Link>
            <Link
              className="button button--outline button--lg button--secondary"
              to="https://github.com/zyvorai/guestkit">
              View on GitHub
            </Link>
          </div>
        </div>
      </div>
    </header>
  );
}

function ProblemStatement() {
  return (
    <section className={styles.problem}>
      <div className="container">
        <Reveal className="row">
          <div className="col col--8 col--offset-2 text--center">
            <Heading as="h2" className={styles.sectionHeading}>
              The cutover problem — solved offline
            </Heading>
            <p>
              Every hypervisor exit fails the same way: you discover the
              disk was broken at 2am, in the cutover window, after
              power-on. GuestKit reads the disk while the guest is off,
              scores first-boot probability 0–100, and emits a reviewable
              fix plan.
            </p>
            <p>
              It's the first step in a suite: <strong>certify</strong> with
              GuestKit, <strong>run &amp; manage</strong> with{' '}
              <Link to="https://github.com/zyvorai/fluxvm">FluxVM</Link>,{' '}
              <strong>convert &amp; deploy</strong> with{' '}
              <Link to="https://github.com/zyvorai/h2kvm">h2kvm</Link>.
              GuestKit itself stays focused on offline intelligence — it
              doesn't own production networking or disposable fleet
              lifecycle.
            </p>
          </div>
        </Reveal>
      </div>
    </section>
  );
}

function TrustBand() {
  return (
    <section className={styles.trust}>
      <div className="container">
        <Reveal className={styles.trustGrid}>
          <div>
            <Heading as="h3" className={styles.sectionHeading}>
              Real, recorded demos
            </Heading>
            <p>
              Apache-2.0 core, 70+ commands across 6 disk formats, 0
              appliance daemons. The CLI &amp; TUI and web dashboard demos
              are recorded live against real deployments — not staged
              screenshots.
            </p>
            <Link to="/docs/user-guides/quick-reference">
              See the full command reference →
            </Link>
          </div>
          <div className={styles.trustBadges}>
            <img
              src="https://github.com/zyvorai/guestkit/actions/workflows/ci.yml/badge.svg"
              alt="CI status"
            />
            <img
              src="https://img.shields.io/crates/v/guestkit.svg"
              alt="crates.io version"
            />
            <img
              src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"
              alt="Apache 2.0 license"
            />
          </div>
        </Reveal>
      </div>
    </section>
  );
}

function EnterpriseCTA() {
  return (
    <section className={styles.enterprise}>
      <div className="container text--center">
        <Reveal>
          <Heading as="h2" className={styles.sectionHeading}>
            Open source core, Enterprise for scale
          </Heading>
          <p className={styles.enterpriseCopy}>
            GuestKit's core is Apache-2.0 and free to run in production.
            Zyvor Enterprise adds a 30-day trial with fleet analysis,
            policy-as-code, and production support for teams running this
            at scale.
          </p>
          <Link
            className="button button--primary button--lg"
            to="https://zyvor.dev/contact?utm_source=github&utm_medium=guestkit&intent=demo">
            Book a demo
          </Link>
        </Reveal>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  return (
    <Layout
      title="GuestKit — offline VM intelligence"
      description="Offline VM intelligence. Migration assurance you can prove. Score boot readiness before power-on, repair disks offline, certify cutover with a Passport.">
      <HomepageHeader />
      <main>
        <ProblemStatement />
        <Reveal>
          <FeatureHighlights />
        </Reveal>
        <TrustBand />
        <EnterpriseCTA />
      </main>
    </Layout>
  );
}
