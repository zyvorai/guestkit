import type {ReactNode} from 'react';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import useBaseUrl from '@docusaurus/useBaseUrl';
import styles from './gallery.module.css';

type Shot = {
  src: string;
  caption: string;
};

type Video = {
  href: string;
  thumb: string;
  title: string;
  blurb: string;
};

// Captured from deploy/ui in its "Try offline demo" mode — see
// scripts/capture-ui-demo.py. Static demo data, not a live deployment.
const CONSOLE: Shot[] = [
  {src: '/ui-00-doctor-demo.png', caption: 'Assurance — doctor score, decision, ranked findings'},
  {src: '/ui-01-inspect-demo.png', caption: 'Summary — OS identity and inventory from inspect'},
  {src: '/ui-02-landing.png', caption: 'Image Vault — import a disk or try the offline demo'},
];

// Recorded live against real deployments.
const VIDEOS: Video[] = [
  {
    href: 'https://www.youtube.com/watch?v=lLEBQoFceIs',
    thumb: 'https://i.ytimg.com/vi/lLEBQoFceIs/maxresdefault.jpg',
    title: '▶ CLI & TUI',
    blurb: 'Offline VM intelligence, explained',
  },
  {
    href: 'https://www.youtube.com/watch?v=usQX2rQIFM8',
    thumb: 'https://i.ytimg.com/vi/usQX2rQIFM8/maxresdefault.jpg',
    title: '▶ Web Dashboard — Overview',
    blurb: 'Server Image Vault, live KubeVirt cluster',
  },
  {
    href: 'https://www.youtube.com/watch?v=icTLVko588A',
    thumb: 'https://i.ytimg.com/vi/icTLVko588A/maxresdefault.jpg',
    title: '▶ Web Dashboard — Deep Dive',
    blurb: 'Sources, live cluster, one-click intelligence',
  },
  {
    href: 'https://www.youtube.com/watch?v=LYoqOye3P3I',
    thumb: '/machina-guestkit-demo-thumb.jpg',
    title: '▶ Machina × GuestKit',
    blurb: 'Live Linux guest agent — health, TRIM, netplan, services',
  },
];

function ShotCard({shot}: {shot: Shot}) {
  const src = useBaseUrl(shot.src);
  return (
    <figure className={styles.shot}>
      <img src={src} alt={shot.caption} loading="lazy" />
      <figcaption>{shot.caption}</figcaption>
    </figure>
  );
}

function VideoCard({video}: {video: Video}) {
  const local = useBaseUrl(video.thumb);
  const thumb = video.thumb.startsWith('/') ? local : video.thumb;
  return (
    <a className={styles.video} href={video.href} target="_blank" rel="noreferrer">
      <img src={thumb} alt={video.title} loading="lazy" />
      <strong>{video.title}</strong>
      <span>{video.blurb}</span>
    </a>
  );
}

export default function Gallery(): ReactNode {
  return (
    <Layout
      title="Gallery"
      description="The GuestKit web console and recorded demos of the CLI, TUI, dashboard and guest agent.">
      <header className={styles.header}>
        <div className="container">
          <Heading as="h1">Product tour</Heading>
          <p>
            The web console below is the bundled OSS UI rendering its offline
            demo data. The recorded demos further down were captured live
            against real deployments.
          </p>
        </div>
      </header>
      <main className="container">
        <div className={styles.grid}>
          {CONSOLE.map((shot) => (
            <ShotCard key={shot.src} shot={shot} />
          ))}
        </div>
        <Heading as="h2" className={styles.sectionTitle}>
          Recorded demos
        </Heading>
        <p className={styles.caption} style={{textAlign: 'center', marginBottom: '1.5rem'}}>
          CLI, TUI, the web dashboard on a KubeVirt cluster, and the guest agent.
        </p>
        <div className={styles.videos}>
          {VIDEOS.map((v) => (
            <VideoCard key={v.href} video={v} />
          ))}
        </div>
      </main>
    </Layout>
  );
}
