import { useState } from "react";

interface Props {
  message: string;
  error: boolean;
}

export function BalanceLabToast({ message, error }: Props) {
  const [dismissed, setDismissed] = useState(false);
  if (dismissed) return null;
  return (
    <div
      className={`toast ${error ? "error" : ""}`}
      role={error ? "alert" : "status"}
      aria-live={error ? "assertive" : "polite"}
    >
      <span>{message}</span>
      <button type="button" aria-label="Dismiss notification" onClick={() => setDismissed(true)}>
        ×
      </button>
    </div>
  );
}
