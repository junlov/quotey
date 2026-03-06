import type {ReactNode} from 'react';
import clsx from 'clsx';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  description: ReactNode;
};

const FeatureList: FeatureItem[] = [
  {
    title: 'Natural Language Quoting',
    description: (
      <>
        Create quotes through natural conversation in Slack. No rigid forms,
        no training required. Just tell Quotey what you need.
      </>
    ),
  },
  {
    title: 'Deterministic Pricing',
    description: (
      <>
        Every price is calculated deterministically with a complete audit trail.
        Know exactly how every number was derived.
      </>
    ),
  },
  {
    title: 'Local-First Architecture',
    description: (
      <>
        Runs entirely on your infrastructure. SQLite database, no cloud
        dependencies, complete data ownership.
      </>
    ),
  },
  {
    title: 'Constraint-Based Config',
    description: (
      <>
        Define what must be true rather than thousands of if-then rules.
        Scales better, easier to maintain.
      </>
    ),
  },
  {
    title: 'Intelligent Approvals',
    description: (
      <>
        Context-aware approval routing with precedent analysis and
        auto-generated justification text.
      </>
    ),
  },
  {
    title: 'Full Audit Trail',
    description: (
      <>
        Every action logged. Complete pricing traces, policy evaluations,
        and decision history for compliance.
      </>
    ),
  },
];

function Feature({title, description}: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center padding-horiz--md">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): ReactNode {
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
