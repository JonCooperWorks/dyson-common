/* Trigger a browser download for an in-memory Blob via a transient
 * object URL and a synthetic anchor click. */

export function downloadBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const revokeObjectURL = URL.revokeObjectURL?.bind(URL);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename || 'download';
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => revokeObjectURL?.(url), 5000);
}
