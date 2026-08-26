interface Props {
  differenceCount: number;
  copied: boolean;
  onCopy: () => void;
}

export function CanonicalDiffSummary({ differenceCount, copied, onCopy }: Props) {
  const plural = differenceCount === 1 ? "value differs" : "values differ";
  return (
    <section className="canonical-diff-summary" aria-label="Server default comparison">
      <div>
        <p className="eyebrow">Server default comparison</p>
        <strong>
          {differenceCount === 0
            ? "Current draft matches the server defaults"
            : `${differenceCount} ${plural} from the server defaults`}
        </strong>
        <small><span aria-hidden="true" /> Non-default values are shown in red.</small>
      </div>
      <button type="button" disabled={differenceCount === 0} onClick={onCopy}>
        {copied ? `Copied ${differenceCount}` : `Copy differences (${differenceCount})`}
      </button>
    </section>
  );
}
