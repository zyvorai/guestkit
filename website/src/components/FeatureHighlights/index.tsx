import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  description: ReactNode;
  to: string;
};

const FeatureList: FeatureItem[] = [
  {
    title: 'Doctor: boot-readiness scoring',
    description:
      'Score first-boot probability 0–100 with a root-cause chain, offline — before you ever power on the guest. Works against qcow2, vmdk, vhdx, vhd, vdi, and raw.',
    to: '/docs/user-guides/getting-started',
  },
  {
    title: 'Migrate-plan + CI gate',
    description:
      'A structured, reviewable fix plan as JSON/YAML, plus a CI gate (guestkit gate --fail-below 80) so a broken disk fails the pipeline, not the cutover window.',
    to: '/docs/user-guides/quick-reference',
  },
  {
    title: 'Cutover Passport',
    description:
      'A signed, verifiable record (guestkit passport emit / verify) — an audit trail for migration that MTV or virt-v2v can otherwise skip entirely.',
    to: '/docs/features/vm-runtime',
  },
  {
    title: 'Assured QEMU launch',
    description:
      'guestkit-qemu plans and runs from the same evidence gate — assured first boot without hand-building QEMU argv yourself.',
    to: '/docs/features/qemu-runtime',
  },
  {
    title: 'h2kvm pipeline integration',
    description:
      'Pre-flight with guestkit doctor, then hand off to h2kvm for hypervisor-to-KVM conversion and deploy — offline repair and conversion in one flow.',
    to: '/docs/features/hyper2kvm-integration',
  },
  {
    title: 'Python bindings + GitHub Action',
    description:
      'Same assurance engine on PyPI (hypersdk-guestkit) for scripting, plus a zyvorai/guestkit@v1 GitHub Action for CI gating with no CLI install step.',
    to: '/docs/user-guides/python-bindings',
  },
];

function Feature({title, description, to}: FeatureItem) {
  return (
    <div className="col col--4">
      <Link to={to} className={styles.card}>
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </Link>
    </div>
  );
}

export default function FeatureHighlights(): ReactNode {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
