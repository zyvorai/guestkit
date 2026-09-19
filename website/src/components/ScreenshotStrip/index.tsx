import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type Shot = {
  src: string;
  alt: string;
};

const SHOTS: Shot[] = [
  {src: '/ui-00-doctor-demo.png', alt: 'Assurance — doctor score 78 with ranked findings'},
  {src: '/ui-01-inspect-demo.png', alt: 'Summary — OS identity and inventory counts'},
  {src: '/ui-02-landing.png', alt: 'Image Vault landing page'},
];

export default function ScreenshotStrip(): ReactNode {
  return (
    <section className={styles.strip}>
      <div className="container">
        <Heading as="h2" className="text--center">
          The web console
        </Heading>
        <p className="text--center">
          The bundled OSS console rendering its offline demo data.{' '}
          <Link to="/gallery">See the tour and recorded demos →</Link>
        </p>
        <div className={styles.grid}>
          {SHOTS.map((shot) => (
            <ShotImage key={shot.src} shot={shot} />
          ))}
        </div>
      </div>
    </section>
  );
}

function ShotImage({shot}: {shot: Shot}) {
  const src = useBaseUrl(shot.src);
  return (
    <Link to="/gallery" className={styles.frame}>
      <img src={src} alt={shot.alt} loading="lazy" />
    </Link>
  );
}
