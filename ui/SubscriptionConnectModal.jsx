import React from 'react';

import { Modal } from './Modal.jsx';

export const SUBSCRIPTION_CONNECT_PROVIDERS = Object.freeze({
  codex: Object.freeze({
    id: 'codex', badge: 'Codex', service: 'ChatGPT', tone: 'codex', flow: 'device',
  }),
  claude: Object.freeze({
    id: 'claude', badge: 'Claude', service: 'Claude', tone: 'claude', flow: 'code',
  }),
});

/**
 * Shared managed-subscription sign-in surface.
 *
 * OAuth credentials never pass through this component. `initial` and
 * `getStatus` receive only the broker's safe status projection; `completeAuth`
 * submits Claude's one-time authorization code directly to that broker.
 */
export function SubscriptionConnectModal({
  initial,
  subscription,
  getStatus,
  completeAuth,
  onConnected,
  onClose,
  subjectLabel = 'this agent',
}) {
  const [auth, setAuth] = React.useState(initial || {});
  const [copied, setCopied] = React.useState(false);
  const [code, setCode] = React.useState('');
  const [submitting, setSubmitting] = React.useState(false);
  const getStatusRef = React.useRef(getStatus);
  const completeAuthRef = React.useRef(completeAuth);
  const onConnectedRef = React.useRef(onConnected);

  getStatusRef.current = getStatus;
  completeAuthRef.current = completeAuth;
  onConnectedRef.current = onConnected;

  React.useEffect(() => {
    let alive = true;
    let completed = false;
    const poll = async () => {
      try {
        const next = await getStatusRef.current?.();
        if (!alive || !next) return;
        setAuth(next);
        if (next.connected && !completed) {
          completed = true;
          try {
            await onConnectedRef.current?.();
          } catch {
            completed = false;
            if (alive) setAuth({ ...next, error: 'Connected, but setup did not finish. Retrying…' });
          }
        }
      } catch {
        // A transient broker failure should not discard a still-valid code.
      }
    };
    const timer = setInterval(poll, 1000);
    poll();
    return () => { alive = false; clearInterval(timer); };
  }, [subscription.id]);

  const copyCode = async () => {
    if (!auth.user_code || !navigator.clipboard?.writeText) return;
    await navigator.clipboard.writeText(auth.user_code);
    setCopied(true);
  };

  const complete = async (event) => {
    event.preventDefault();
    if (!code.trim() || typeof completeAuthRef.current !== 'function') return;
    setSubmitting(true);
    try {
      const next = await completeAuthRef.current(code.trim());
      setAuth(next || {});
      if (next?.connected) await onConnectedRef.current?.();
    } catch {
      setAuth(previous => ({
        ...previous,
        error: 'That authorization code could not be accepted. Check the full code and try again.',
      }));
    } finally {
      setSubmitting(false);
    }
  };

  const signInUrl = auth.auth_url || auth.verification_uri;
  const isFailed = auth.state === 'failed' || auth.state === 'unavailable';

  return (
    <Modal
      className={`modal subscription-connect subscription-connect-${subscription.tone}`}
      scrimClassName="modal-scrim subscription-connect-scrim"
      label={`Connect ${subscription.service} subscription`}
      onClose={onClose}
    >
      <div className="subscription-connect-head">
        <div className="subscription-connect-brand">
          <span className={`subscription-backend-badge subscription-backend-badge-${subscription.tone}`}>
            <span className="subscription-backend-mark" aria-hidden="true"/>
            {subscription.badge}
          </span>
          <span>Subscription access</span>
        </div>
        <button type="button" className="subscription-connect-close" onClick={onClose} aria-label="Close">×</button>
      </div>
      <div className="subscription-connect-body">
        <div className="subscription-connect-kicker">Native provider</div>
        <h2>Connect {subscription.service}</h2>
        <p className="subscription-connect-lede">
          Sign in once, then use your {subscription.service} plan directly from {subjectLabel}.
        </p>

        <div className="subscription-privacy">
          <span className="subscription-privacy-mark" aria-hidden="true">✓</span>
          <div>
            <strong>Credentials stay in Swarm.</strong>
            <span>{subjectLabel} receives only an instance-bound proxy token.</span>
          </div>
        </div>

        {signInUrl ? (
          <div className="subscription-connect-flow">
            <div className="subscription-step">
              <span className="subscription-step-number">1</span>
              <div>
                <strong>Open {subscription.service}</strong>
                <span>Approve access in a secure provider window.</span>
              </div>
              <a className="subscription-connect-action subscription-connect-primary" href={signInUrl} target="_blank" rel="noreferrer">
                Open sign-in <span aria-hidden="true">↗</span>
              </a>
            </div>

            {subscription.flow === 'device' && auth.user_code ? (
              <div className="subscription-step">
                <span className="subscription-step-number">2</span>
                <div>
                  <strong>Enter this code</strong>
                  <span>The window will ask for it.</span>
                </div>
                <button type="button" className="subscription-device-code" onClick={copyCode} aria-label="Copy device code">
                  <span className="subscription-code-value">{auth.user_code}</span>
                  <span>{copied ? 'Copied' : 'Copy'}</span>
                </button>
              </div>
            ) : null}

            {subscription.flow === 'code' ? (
              <form className="subscription-step subscription-code-step" onSubmit={complete}>
                <span className="subscription-step-number">2</span>
                <label>
                  <strong>Paste the returned code</strong>
                  <span>Copy the complete code from Claude after approval.</span>
                  <input
                    value={code}
                    onChange={event => setCode(event.target.value)}
                    autoComplete="off"
                    spellCheck="false"
                    placeholder="Authorization code"
                  />
                </label>
                <button type="submit" className="subscription-connect-action subscription-connect-primary" disabled={!code.trim() || submitting}>
                  {submitting ? 'Connecting…' : 'Connect'}
                </button>
              </form>
            ) : null}
          </div>
        ) : isFailed ? (
          <p className="subscription-connect-error">{auth.error || `${subscription.service} sign-in could not start.`}</p>
        ) : (
          <div className="subscription-starting"><span/>Preparing secure sign-in…</div>
        )}
        {auth.error && !isFailed ? <p className="subscription-connect-error">{auth.error}</p> : null}
      </div>
    </Modal>
  );
}
