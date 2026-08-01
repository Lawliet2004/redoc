export function debounce<T extends (...args: any[]) => void>(fn: T, delay: number): T {
  let timer: any = null;
  return ((...args: any[]) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => fn(...args), delay);
  }) as T;
}

export function throttle<T extends (...args: any[]) => void>(fn: T, limit: number): T {
  let inThrottle: boolean = false;
  return ((...args: any[]) => {
    if (!inThrottle) {
      fn(...args);
      inThrottle = true;
      setTimeout(() => (inThrottle = false), limit);
    }
  }) as T;
}

export function generateUuid(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

export function formatDate(timestampSecs: number): string {
  const date = new Date(timestampSecs * 1000);
  return date.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
export function encodeBase64(str: string): string {
  return btoa(encodeURIComponent(str).replace(/%([0-9A-F]{2})/g, function(match, p1) {
    return String.fromCharCode(parseInt(p1, 16));
  }));
}
export function decodeBase64(str: string): string {
  return decodeURIComponent(atob(str).split('').map(function(c) {
    return '%' + ('00' + c.charCodeAt(0).toString(16)).slice(-2);
  }).join(''));
}
export function parseTsv(tsv: string): string[][] {
  return tsv.split('\n').map(row => row.split('\t'));
}
export function serializeTsv(data: string[][]): string {
  return data.map(row => row.join('\t')).join('\n');
}
