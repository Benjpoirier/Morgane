export function formatDuration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  if (total < 60) return `${total} s`;
  const minutes = Math.round(total / 60);
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
}

export type Estimator = { done: number; at: number; rate: number | null; samples: number };

const SMOOTHING = 0.3;

const MIN_SAMPLE_SECONDS = 0.05;

export function freshEstimator(): Estimator {
  return { done: 0, at: 0, rate: null, samples: 0 };
}

export function observe(est: Estimator, done: number, now: number): number | null {
  if (est.at === 0) {
    est.done = done;
    est.at = now;
    return null;
  }
  const elapsed = (now - est.at) / 1000;
  if (elapsed < MIN_SAMPLE_SECONDS || done <= est.done) return null;

  const instant = (done - est.done) / elapsed;
  est.rate = est.rate === null ? instant : est.rate * (1 - SMOOTHING) + instant * SMOOTHING;
  est.samples += 1;
  est.done = done;
  est.at = now;
  return est.samples >= 2 && est.rate > 0 ? est.rate : null;
}

export function remainingSeconds(rate: number | null, done: number, total: number): number | null {
  if (rate === null || rate <= 0 || total <= 0 || done >= total) return null;
  return (total - done) / rate;
}
